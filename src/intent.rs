//! Authoritative project intent, architectural directives, and invariant requirements.
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{display::safe, history, sensitive, validate_text};

#[derive(Subcommand, Debug)]
pub enum Action {
    /// List active project intent directives and permanent invariants.
    List,
    /// Declare an authoritative project intent directive or invariant.
    Add {
        text: String,
        /// Explicit key grouping competing directives or intent domains.
        #[arg(long)]
        key: Option<String>,
        /// Self-reported actor label (default: architect).
        #[arg(long, default_value = "architect")]
        actor: String,
        /// Origin source label (default: project-intent).
        #[arg(long, default_value = "project-intent")]
        source: String,
        #[arg(long)]
        allow_sensitive: bool,
    },
    /// Promote an existing memory requirement to a permanent intent directive.
    Pin { id: i64 },
    /// Gracefully retire an intent directive with an explicit invalidation reason.
    Retire {
        id: i64,
        #[arg(long)]
        reason: String,
    },
}

impl Action {
    pub fn writable(&self) -> bool {
        matches!(
            self,
            Self::Add { .. } | Self::Pin { .. } | Self::Retire { .. }
        )
    }
}

pub fn run(conn: &mut Connection, project: i64, action: Action) -> Result<()> {
    match action {
        Action::List => list(conn, project)?,
        Action::Add {
            text,
            key,
            actor,
            source,
            allow_sensitive,
        } => add(
            conn,
            project,
            &text,
            key.as_deref(),
            &actor,
            &source,
            allow_sensitive,
        )?,
        Action::Pin { id } => pin(conn, project, id)?,
        Action::Retire { id, reason } => retire(conn, project, id, &reason)?,
    }
    Ok(())
}

fn list(conn: &Connection, project: i64) -> Result<()> {
    let modern = crate::storage::version(conn)? >= 2;
    let provenance = crate::storage::version(conn)? >= 3;

    let validity_clause = if provenance {
        "AND coalesce(d.validity, 'valid') = 'valid'"
    } else {
        ""
    };
    let active_clause = if modern {
        "AND coalesce(d.state, 'active') = 'active'"
    } else {
        ""
    };
    let metadata_join = if modern {
        "LEFT JOIN memory_metadata d ON d.memory_id = m.id"
    } else {
        ""
    };
    let origins_join = if provenance {
        "LEFT JOIN memory_origins o ON o.memory_id = m.id"
    } else {
        ""
    };

    let metadata = if modern {
        "d.memory_key, d.expires_at"
    } else {
        "NULL, NULL"
    };
    let origins = if provenance {
        "o.source, o.actor"
    } else {
        "NULL, NULL"
    };
    let order = if modern {
        "(d.expires_at IS NULL) DESC, m.id ASC"
    } else {
        "m.id ASC"
    };
    let sql = format!(
        "SELECT m.id, m.content, m.created_at, {metadata}, {origins}
         FROM memories m
         {metadata_join}
         {origins_join}
         WHERE m.project_id = ?1
           AND (m.kind = 'requirement' COLLATE NOCASE OR m.kind = 'intent' COLLATE NOCASE)
           {active_clause}
           {validity_clause}
         ORDER BY {order}"
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![project])?;
    let mut count = 0;

    println!("Project Intent & Invariants\n");
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let content: String = row.get(1)?;
        let created: String = row.get(2)?;
        let key: Option<String> = row.get(3)?;
        let expires_at: Option<i64> = row.get(4)?;
        let source: Option<String> = row.get(5)?;
        let actor: Option<String> = row.get(6)?;

        let is_pinned = expires_at.is_none();
        let key_str = key
            .map(|k| format!(" key=\"{}\"", sensitive::display(&k, false)))
            .unwrap_or_default();
        let prov_str = match (source, actor) {
            (Some(s), Some(a)) => format!(
                " [by {} via {}]",
                sensitive::display(&a, false),
                sensitive::display(&s, false)
            ),
            _ => String::new(),
        };

        println!(
            "#{id} [{}] {}{}{}",
            if is_pinned {
                "INVARIANT"
            } else {
                "REQUIREMENT"
            },
            safe(&created),
            key_str,
            prov_str
        );
        println!("  {}", sensitive::display(&content, false));
        count += 1;
    }

    if count == 0 {
        println!("No active intent directives recorded. Use 'swapctx intent add <text>' to preserve project intent.");
    } else {
        println!("\nTotal active intent directives: {count}");
    }

    Ok(())
}

fn add(
    conn: &mut Connection,
    project: i64,
    text: &str,
    key: Option<&str>,
    actor: &str,
    source: &str,
    allow_sensitive: bool,
) -> Result<()> {
    validate_text("Intent content", text, 1024 * 1024)?;
    sensitive::guard(text, allow_sensitive)?;
    if let Some(key) = key {
        validate_text("Intent key", key, 128)?;
        sensitive::guard(key, allow_sensitive)?;
    }
    for value in [actor, source] {
        validate_text("Provenance label", value, 4096)?;
        sensitive::guard(value, allow_sensitive)?;
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Save as 'requirement' so that context.rs treats it as mandatory context!
    tx.execute(
        "INSERT INTO memories(project_id, kind, content) VALUES (?1, 'requirement', ?2)",
        params![project, text],
    )?;
    let id = tx.last_insert_rowid();

    let now = crate::memory::now()?;
    // Intent is non-expiring by default (expires_at = NULL)
    tx.execute(
        "INSERT INTO memory_metadata(memory_id, memory_key, expires_at, reviewed_at, state, validity)
         VALUES (?1, ?2, NULL, ?3, 'active', 'valid')",
        params![id, key, now],
    )?;

    if crate::storage::version(&tx)? >= 3 {
        history::origin(&tx, id, source, actor)?;
        history::event(&tx, id, "add", "Authoritative project invariant declared")?;
    }

    tx.commit()?;
    println!("Preserved project intent directive #{id}");
    Ok(())
}

fn pin(conn: &mut Connection, project: i64, id: i64) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let ver = crate::storage::version(&tx)?;
    if ver < 2 {
        bail!("Schema version 1 does not support metadata pinning; upgrade database schema");
    }

    let row = tx
        .query_row(
            "SELECT m.kind, coalesce(d.state, 'active'), coalesce(d.validity, 'valid')
         FROM memories m
         LEFT JOIN memory_metadata d ON d.memory_id = m.id
         WHERE m.id = ?1 AND m.project_id = ?2",
            params![id, project],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .context("Memory record not found in this project")?;

    let (kind, state, validity) = row;

    // Restrict pin operations to valid, active lifecycle records of requirement or intent kind
    if validity != "valid" {
        bail!("Cannot pin memory #{id}: record is currently marked invalid");
    }
    if state != "active" {
        bail!("Cannot pin memory #{id}: record is currently in '{state}' state (only active records can be pinned)");
    }
    if !kind.eq_ignore_ascii_case("requirement") && !kind.eq_ignore_ascii_case("intent") {
        bail!(
            "Cannot pin memory #{id}: only 'requirement' records can be promoted to permanent project intent directives (found '{kind}')"
        );
    }

    let now = crate::memory::now()?;
    // Clear expiration to make it a permanent invariant
    tx.execute(
        "INSERT INTO memory_metadata(memory_id, expires_at, reviewed_at, state, validity)
         VALUES (?1, NULL, ?2, 'active', 'valid')
         ON CONFLICT(memory_id) DO UPDATE SET expires_at = NULL, reviewed_at = excluded.reviewed_at",
        params![id, now],
    )?;

    if ver >= 3 {
        history::event(
            &tx,
            id,
            "review",
            "Promoted to permanent project intent directive",
        )?;
    }

    tx.commit()?;
    println!("Pinned memory #{id} as permanent project intent directive (expiry cleared)");
    Ok(())
}

fn retire(conn: &mut Connection, project: i64, id: i64, reason: &str) -> Result<()> {
    validate_text("Retirement reason", reason, 16384)?;
    sensitive::guard(reason, false)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let ver = crate::storage::version(&tx)?;
    if ver < 2 {
        bail!("Schema version 1 does not support invalidation tracking; upgrade database schema");
    }

    let row = tx
        .query_row(
            "SELECT m.kind, coalesce(d.state, 'active'), coalesce(d.validity, 'valid')
         FROM memories m
         LEFT JOIN memory_metadata d ON d.memory_id = m.id
         WHERE m.id = ?1 AND m.project_id = ?2",
            params![id, project],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .context("Memory record not found in this project")?;

    let (kind, state, validity) = row;

    if validity == "invalid" {
        bail!("Memory #{id} is already retired / invalid");
    }
    if state != "active" {
        bail!("Cannot retire memory #{id}: record is currently in '{state}' state");
    }
    if !kind.eq_ignore_ascii_case("requirement") && !kind.eq_ignore_ascii_case("intent") {
        bail!(
            "Cannot retire memory #{id}: only project intent / requirement directives can be retired (found '{kind}')"
        );
    }

    tx.execute(
        "INSERT INTO memory_metadata(memory_id, validity, invalid_reason)
         VALUES (?1, 'invalid', ?2)
         ON CONFLICT(memory_id) DO UPDATE SET validity = 'invalid', invalid_reason = excluded.invalid_reason",
        params![id, reason],
    )?;

    if ver >= 3 {
        history::event(&tx, id, "invalidate", reason)?;
    }

    tx.commit()?;
    println!("Retired project intent directive #{id} (invalidated with reason retained)");
    Ok(())
}
