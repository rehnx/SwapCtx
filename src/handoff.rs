//! Safe, developer-approved transfer of verified context and project intent between agents.
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{display::safe, history, paths, sensitive, validate_text};

#[derive(Subcommand, Debug)]
pub enum Action {
    /// Export approved project knowledge and intent into a portable handoff bundle.
    Export {
        destination: PathBuf,
        /// Include active boundary protection rules in the handoff bundle.
        #[arg(long)]
        include_rules: bool,
        /// Label for the receiving agent or target purpose (default: agent).
        #[arg(long, default_value = "agent")]
        target: String,
    },
    /// Preview knowledge in a handoff bundle without applying changes.
    Preview { source: PathBuf },
    /// Transactionally apply approved handoff knowledge to current project.
    Apply {
        source: PathBuf,
        /// Allow potentially sensitive values if any exist in the bundle.
        #[arg(long)]
        allow_sensitive: bool,
    },
}

impl Action {
    pub fn writable(&self) -> bool {
        matches!(self, Self::Apply { .. })
    }
}

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

pub fn run(conn: &mut Connection, project: i64, root: &Path, action: Action) -> Result<()> {
    match action {
        Action::Export {
            destination,
            include_rules,
            target,
        } => export(conn, project, root, &destination, include_rules, &target)?,
        Action::Preview { source } => preview(conn, project, &source)?,
        Action::Apply {
            source,
            allow_sensitive,
        } => apply(conn, project, &source, allow_sensitive)?,
    }
    Ok(())
}

fn export(
    conn: &Connection,
    project: i64,
    _root: &Path,
    destination: &Path,
    include_rules: bool,
    target: &str,
) -> Result<()> {
    validate_text("Target label", target, 128)?;
    sensitive::guard(target, false)?;

    let now = crate::memory::now()?;
    let modern = crate::storage::version(conn)? >= 2;
    let provenance = crate::storage::version(conn)? >= 3;

    // Collect approved, active, valid, fresh, unconflicted requirements & intent
    let validity_clause = if provenance {
        "AND coalesce(d.validity, 'valid') = 'valid'"
    } else {
        ""
    };
    let active_clause = if modern {
        "AND coalesce(d.state, 'active') = 'active' AND (d.expires_at IS NULL OR d.expires_at > ?2)"
    } else {
        ""
    };
    let unconflicted_clause = if modern {
        "AND (d.memory_key IS NULL OR d.memory_key NOT IN (
            SELECT c.memory_key FROM memory_metadata c
            JOIN memories cm ON cm.id = c.memory_id
            WHERE cm.project_id = ?1 AND c.memory_key IS NOT NULL
              AND coalesce(c.state, 'active') = 'active'
              AND coalesce(c.validity, 'valid') = 'valid'
            GROUP BY c.memory_key HAVING count(*) > 1
        ))"
    } else {
        ""
    };

    let sql = format!(
        "SELECT m.id, m.kind, m.content, d.memory_key, o.source, o.actor
         FROM memories m
         LEFT JOIN memory_metadata d ON d.memory_id = m.id
         LEFT JOIN memory_origins o ON o.memory_id = m.id
         WHERE m.project_id = ?1
           AND m.kind IN ('requirement', 'intent')
           {active_clause}
           {validity_clause}
           {unconflicted_clause}
         ORDER BY m.id ASC"
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![project, now])?;
    let mut memories = Vec::new();

    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        let kind: String = r.get(1)?;
        let content: String = r.get(2)?;
        let key: Option<String> = r.get(3)?;
        let source: Option<String> = r.get(4)?;
        let actor: Option<String> = r.get(5)?;

        // Never export credentials or secrets
        if [
            &kind,
            &content,
            key.as_deref().unwrap_or(""),
            source.as_deref().unwrap_or(""),
            actor.as_deref().unwrap_or(""),
        ]
        .iter()
        .any(|s| sensitive::guard(s, false).is_err())
        {
            continue;
        }

        memories.push(format!(
            "{{\"id\":{},\"kind\":\"{}\",\"content\":\"{}\",\"key\":\"{}\",\"source\":\"{}\",\"actor\":\"{}\"}}",
            id,
            json_escape(&kind),
            json_escape(&content),
            json_escape(key.as_deref().unwrap_or("")),
            json_escape(source.as_deref().unwrap_or("approved-handoff")),
            json_escape(actor.as_deref().unwrap_or("developer"))
        ));
    }

    let mut rules = Vec::new();
    if include_rules {
        let mut r_stmt = conn.prepare(
            "SELECT r.pattern, r.reason, coalesce(p.policy, 'deny')
             FROM rules r
             LEFT JOIN rule_policies p ON p.rule_id = r.id
             WHERE r.project_id = ?1
             ORDER BY r.id ASC",
        )?;
        let mut r_rows = r_stmt.query(params![project])?;
        while let Some(r) = r_rows.next()? {
            let pattern: String = r.get(0)?;
            let reason: String = r.get(1)?;
            let policy: String = r.get(2)?;

            if [&pattern, &reason, &policy]
                .iter()
                .any(|s| sensitive::guard(s, false).is_err())
            {
                continue;
            }

            rules.push(format!(
                "{{\"pattern\":\"{}\",\"reason\":\"{}\",\"policy\":\"{}\"}}",
                json_escape(&pattern),
                json_escape(&reason),
                json_escape(&policy)
            ));
        }
    }

    let mem_str = format!("[{}]", memories.join(","));
    let rule_str = format!("[{}]", rules.join(","));

    let json_bundle: String = conn.query_row(
        "SELECT json_object(
            'protocol', 'swapctx.handoff',
            'version', 1,
            'timestamp', ?1,
            'target', ?2,
            'items_count', ?3,
            'rules_count', ?4,
            'memories', json(?5),
            'rules', json(?6)
        )",
        params![
            now,
            target,
            memories.len() as i64,
            rules.len() as i64,
            mem_str,
            rule_str
        ],
        |r| r.get(0),
    )?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    // Atomic file creation to prevent overwrite and symlink races
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .context(
            "Destination file already exists or symlink race detected; refusing to overwrite",
        )?;

    file.write_all(json_bundle.as_bytes())?;
    file.flush()?;

    println!(
        "Created agent handoff bundle: {}",
        safe(&destination.display().to_string())
    );
    println!("  - Approved requirements & intent: {}", memories.len());
    println!("  - Boundary rules included:       {}", rules.len());
    println!(
        "  - Target recipient:              {}",
        sensitive::display(target, false)
    );
    println!("  - Credentials / secrets scrubbed: verified");

    Ok(())
}

fn preview(conn: &Connection, project: i64, source: &Path) -> Result<()> {
    if !source.is_file() {
        bail!("Handoff bundle file not found: {}", source.display());
    }

    let content = fs::read_to_string(source)?;
    let parser = rusqlite::Connection::open_in_memory()?;

    let (protocol, version, target, items_count, rules_count): (String, i64, String, i64, i64) =
        parser
            .query_row(
                "SELECT
            json_extract(?1, '$.protocol'),
            json_extract(?1, '$.version'),
            coalesce(json_extract(?1, '$.target'), 'general'),
            coalesce(json_extract(?1, '$.items_count'), 0),
            coalesce(json_extract(?1, '$.rules_count'), 0)",
                [&content],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .context("Invalid handoff bundle JSON format")?;

    if protocol != "swapctx.handoff" || version != 1 {
        bail!("Unsupported handoff bundle protocol or version");
    }

    println!("Previewing Agent Knowledge Handoff Bundle\n");
    println!("Source:       {}", safe(&source.display().to_string()));
    println!("Target agent: {}", sensitive::display(&target, false));
    println!("Requirements: {items_count} approved entries");
    println!("Rules:        {rules_count} boundary rules\n");

    let mut stmt = parser.prepare(
        "SELECT json_extract(value, '$.id'), json_extract(value, '$.kind'),
                json_extract(value, '$.content'), json_extract(value, '$.key')
             FROM json_each(?1, '$.memories')",
    )?;
    let mut rows = stmt.query([&content])?;
    let mut shown = 0;
    let mut exact_matches = 0;
    let mut key_conflicts = 0;
    let mut new_entries = 0;

    while let Some(r) = rows.next()? {
        let orig_id: i64 = r.get(0)?;
        let kind: String = r.get(1)?;
        let text: String = r.get(2)?;
        let key: Option<String> = r.get(3)?;
        let clean_key = key.as_deref().filter(|k| !k.is_empty());

        // Structural context-record matching against current project
        let exact_match: Option<i64> = conn
            .query_row(
                "SELECT m.id FROM memories m
                 LEFT JOIN memory_metadata d ON d.memory_id = m.id
                 WHERE m.project_id = ?1 AND m.kind = ?2 AND m.content = ?3
                   AND coalesce(d.memory_key, '') = ?4
                   AND coalesce(d.state, 'active') = 'active'
                   AND coalesce(d.validity, 'valid') = 'valid'",
                params![project, kind, text, clean_key.unwrap_or("")],
                |r| r.get(0),
            )
            .optional()?;

        // Conflict check: existing active record with same key but different content
        let conflict_existing: Option<i64> = if let Some(k) = clean_key {
            conn.query_row(
                "SELECT m.id FROM memories m
                 JOIN memory_metadata d ON d.memory_id = m.id
                 WHERE m.project_id = ?1 AND d.memory_key = ?2
                   AND coalesce(d.state, 'active') = 'active'
                   AND coalesce(d.validity, 'valid') = 'valid'
                   AND m.content != ?3",
                params![project, k, text],
                |r| r.get(0),
            )
            .optional()?
        } else {
            None
        };

        let status_badge = if let Some(cid) = conflict_existing {
            key_conflicts += 1;
            format!(
                " [CONFLICT: key \"{}\" competes with existing #{cid}]",
                sensitive::display(clean_key.unwrap_or(""), false)
            )
        } else if let Some(id) = exact_match {
            exact_matches += 1;
            format!(" [EXISTS: exact match #{id}]")
        } else {
            new_entries += 1;
            " [NEW]".to_string()
        };

        let key_str = clean_key
            .map(|k| format!(" key=\"{}\"", sensitive::display(k, false)))
            .unwrap_or_default();

        if shown < 5 {
            println!(
                "  - [origin #{orig_id}] [{}]{}{}",
                sensitive::display(&kind, false),
                key_str,
                status_badge
            );
            println!("      {}", sensitive::display(&text, false));
            shown += 1;
        }
    }

    if items_count > 5 {
        println!("      ... and {} more entries", items_count - 5);
    }

    // Preview boundary rules if present
    let mut r_stmt = parser.prepare(
        "SELECT json_extract(value, '$.pattern'), json_extract(value, '$.reason'),
                json_extract(value, '$.policy')
         FROM json_each(?1, '$.rules')",
    )?;
    let mut r_rows = r_stmt.query([&content])?;
    let mut shown_rules = 0;
    let mut rule_conflicts = 0;
    while let Some(r) = r_rows.next()? {
        let pattern: String = r.get(0)?;
        let reason: String = r.get(1)?;
        let policy: String = r.get(2)?;
        let norm_policy = policy.trim().to_ascii_lowercase();

        let existing_rule: Option<(i64, String, String)> = conn
            .query_row(
                "SELECT r.id, r.reason, coalesce(p.policy, 'deny')
                 FROM rules r
                 LEFT JOIN rule_policies p ON p.rule_id = r.id
                 WHERE r.project_id = ?1 AND r.pattern = ?2",
                params![project, pattern],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let status_str = if let Some((_r_id, ex_reason, ex_pol)) = existing_rule {
            let ex_pol_norm = ex_pol.trim().to_ascii_lowercase();
            if ex_pol_norm == norm_policy && ex_reason == reason {
                " [EXISTS: exact match]"
            } else {
                rule_conflicts += 1;
                " [CONFLICT: pattern collides with differing policy/reason]"
            }
        } else {
            " [NEW]"
        };

        if shown_rules < 5 {
            println!(
                "  - [rule] pattern=\"{}\" policy={} reason=\"{}\"{}",
                sensitive::display(&pattern, false),
                sensitive::display(&norm_policy, false),
                sensitive::display(&reason, false),
                status_str
            );
            shown_rules += 1;
        }
    }
    if rules_count > 5 {
        println!("      ... and {} more rules", rules_count - 5);
    }

    println!("\nStructural Matching & Conflict Analysis:");
    println!("  - Structurally identical records (already present): {exact_matches}");
    println!("  - Competing key conflicts detected:                 {key_conflicts}");
    println!("  - Conflicting rule patterns detected:               {rule_conflicts}");
    println!("  - New records ready to import:                      {new_entries}");

    println!(
        "\nTo apply this handoff to current project: swapctx handoff apply {}",
        source.display()
    );
    Ok(())
}

fn apply(conn: &mut Connection, project: i64, source: &Path, allow_sensitive: bool) -> Result<()> {
    if !source.is_file() {
        bail!("Handoff bundle file not found: {}", source.display());
    }

    let content = fs::read_to_string(source)?;
    let parser = rusqlite::Connection::open_in_memory()?;

    let (protocol, version, target): (String, i64, String) = parser
        .query_row(
            "SELECT
            json_extract(?1, '$.protocol'),
            json_extract(?1, '$.version'),
            coalesce(json_extract(?1, '$.target'), 'agent')",
            [&content],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .context("Invalid handoff bundle JSON")?;

    if protocol != "swapctx.handoff" || version != 1 {
        bail!("Unsupported handoff bundle protocol or version");
    }

    validate_text("Target label", &target, 128)?;
    sensitive::guard(&target, allow_sensitive)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = crate::memory::now()?;
    let modern = crate::storage::version(&tx)? >= 2;
    let provenance = crate::storage::version(&tx)? >= 3;

    let mut imported_memories = 0;
    let mut already_present = 0;
    let mut imported_rules = 0;
    let mut seen_bundle_keys = std::collections::HashMap::new();

    let mut stmt = parser.prepare(
        "SELECT json_extract(value, '$.kind'), json_extract(value, '$.content'),
                json_extract(value, '$.key'), json_extract(value, '$.source'),
                json_extract(value, '$.actor')
         FROM json_each(?1, '$.memories')",
    )?;
    let mut rows = stmt.query([&content])?;

    while let Some(r) = rows.next()? {
        let kind: String = r.get(0)?;
        let text: String = r.get(1)?;
        let key: Option<String> = r.get(2)?;
        let orig_source: Option<String> = r.get(3)?;
        let orig_actor: Option<String> = r.get(4)?;

        // Rigorous validation of incoming memory fields
        validate_text("Memory kind", &kind, 64)?;
        match kind.as_str() {
            "requirement" | "decision" | "note" | "intent" => (),
            _ => bail!("Invalid memory kind in bundle"),
        }

        validate_text("Memory content", &text, 1024 * 1024)?;
        sensitive::guard(&text, allow_sensitive)?;

        if let Some(k) = &key {
            if !k.is_empty() {
                validate_text("Memory key", k, 128)?;
                sensitive::guard(k, allow_sensitive)?;
            }
        }
        if let Some(s) = &orig_source {
            validate_text("Memory source", s, 4096)?;
            sensitive::guard(s, allow_sensitive)?;
        }
        if let Some(a) = &orig_actor {
            validate_text("Memory actor", a, 4096)?;
            sensitive::guard(a, allow_sensitive)?;
        }

        let clean_key = key.as_deref().filter(|k| !k.is_empty());

        // Check for duplicate competing keys inside the bundle itself
        if let Some(k) = clean_key {
            if let Some(prev_text) = seen_bundle_keys.get(k) {
                if prev_text != &text {
                    bail!(
                        "Bundle contains internal key conflict: multiple entries share a key with differing content"
                    );
                }
            } else {
                seen_bundle_keys.insert(k.to_string(), text.clone());
            }
        }

        // Structural context-record matching: check if exact record already exists
        let exact_exists: bool = tx.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM memories m
                 LEFT JOIN memory_metadata d ON d.memory_id = m.id
                 WHERE m.project_id = ?1 AND m.kind = ?2 AND m.content = ?3
                   AND coalesce(d.memory_key, '') = ?4
                   AND coalesce(d.state, 'active') = 'active'
                   AND coalesce(d.validity, 'valid') = 'valid'
             )",
            params![project, kind, text, clean_key.unwrap_or("")],
            |row| row.get(0),
        )?;

        // Handoff conflict detection: reject atomically if incoming key competes with an existing active record
        if let Some(k) = clean_key {
            let conflict_id: Option<i64> = tx
                .query_row(
                    "SELECT m.id FROM memories m
                     JOIN memory_metadata d ON d.memory_id = m.id
                     WHERE m.project_id = ?1 AND d.memory_key = ?2
                       AND coalesce(d.state, 'active') = 'active'
                       AND coalesce(d.validity, 'valid') = 'valid'
                       AND m.content != ?3
                     LIMIT 1",
                    params![project, k, text],
                    |cr| cr.get(0),
                )
                .optional()?;

            if let Some(conf_id) = conflict_id {
                bail!(
                    "Handoff rejected due to key conflict: incoming key competes with active record #{conf_id} in current project"
                );
            }
        }

        if exact_exists {
            already_present += 1;
            continue;
        }
        tx.execute(
            "INSERT INTO memories(project_id, kind, content) VALUES (?1, ?2, ?3)",
            params![project, kind, text],
        )?;
        let new_id = tx.last_insert_rowid();

        if modern {
            tx.execute(
                "INSERT INTO memory_metadata(memory_id, memory_key, state, validity, reviewed_at)
                 VALUES (?1, ?2, 'active', 'valid', ?3)",
                params![new_id, clean_key, now],
            )?;
        }

        if provenance {
            let src_label = format!(
                "handoff:{target}:{}",
                orig_source.as_deref().unwrap_or("transfer")
            );
            let actor_label = orig_actor.as_deref().unwrap_or("agent-handoff");
            history::origin(&tx, new_id, &src_label, actor_label)?;
            history::event(
                &tx,
                new_id,
                "import",
                "Imported from approved agent handoff bundle",
            )?;
        }

        imported_memories += 1;
    }

    // Import and rigorously validate boundary rules if present
    let mut r_stmt = parser.prepare(
        "SELECT json_extract(value, '$.pattern'), json_extract(value, '$.reason'),
                json_extract(value, '$.policy')
         FROM json_each(?1, '$.rules')",
    )?;
    let mut r_rows = r_stmt.query([&content])?;
    let mut seen_bundle_rule_patterns = std::collections::HashMap::new();

    while let Some(r) = r_rows.next()? {
        let pattern: String = r.get(0)?;
        let reason: String = r.get(1)?;
        let policy: String = r.get(2)?;

        validate_text("Rule pattern", &pattern, 4096)?;
        paths::rule_matcher(&pattern)?;
        sensitive::guard(&pattern, allow_sensitive)?;

        validate_text("Rule reason", &reason, 16384)?;
        sensitive::guard(&reason, allow_sensitive)?;

        let norm_policy = policy.trim().to_ascii_lowercase();
        if !matches!(norm_policy.as_str(), "deny" | "ask" | "audit") {
            bail!("Invalid rule policy in handoff bundle; must be deny, ask, or audit");
        }

        // Check for duplicate rule pattern conflicts inside the bundle
        if let Some((prev_pol, prev_reason)) = seen_bundle_rule_patterns.get(&pattern) {
            if prev_pol != &norm_policy || prev_reason != &reason {
                bail!("Bundle contains conflicting rule definitions for a pattern");
            }
        } else {
            seen_bundle_rule_patterns
                .insert(pattern.clone(), (norm_policy.clone(), reason.clone()));
        }

        // Check if an existing rule with this pattern already exists in project
        let existing_rule: Option<(i64, String, String)> = tx
            .query_row(
                "SELECT r.id, r.reason, coalesce(p.policy, 'deny')
                 FROM rules r
                 LEFT JOIN rule_policies p ON p.rule_id = r.id
                 WHERE r.project_id = ?1 AND r.pattern = ?2",
                params![project, pattern],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        if let Some((rule_id, ex_reason, ex_policy)) = existing_rule {
            let ex_policy_norm = ex_policy.trim().to_ascii_lowercase();
            if ex_policy_norm == norm_policy && ex_reason == reason {
                // Exact structural rule match already exists, skip
                continue;
            } else {
                // Rule collision with differing policy or reason
                bail!(
                    "Handoff rejected due to rule conflict: incoming rule conflicts with existing rule #{rule_id}"
                );
            }
        }

        tx.execute(
            "INSERT INTO rules(project_id, pattern, reason) VALUES (?1, ?2, ?3)",
            params![project, pattern, reason],
        )?;
        let rule_id = tx.last_insert_rowid();

        if modern {
            tx.execute(
                "INSERT INTO rule_policies(rule_id, policy) VALUES (?1, ?2)",
                params![rule_id, norm_policy],
            )?;
        }
        imported_rules += 1;
    }

    tx.commit()?;
    println!("Successfully applied agent handoff bundle:");
    println!("  - Imported requirements & intent: {imported_memories}");
    println!("  - Structurally matched (already present): {already_present}");
    println!("  - Imported boundary rules:       {imported_rules}");
    println!("  - Transaction committed locally");

    Ok(())
}
