//! Investigate context delivery decisions, candidate ranking and omissions.
use anyhow::{Context, Result};
use clap::Args;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use crate::{display::safe, paths, sensitive};

#[derive(Args, Debug)]
pub struct Options {
    /// Explain context delivery decisions for a specific memory record ID.
    #[arg(long)]
    pub id: Option<i64>,
    /// Explain boundary rules and requirements governing a specific file path.
    #[arg(long)]
    pub path: Option<String>,
    /// Explain candidate ranking and budget allocation for a query.
    #[arg(long)]
    pub query: Option<String>,
    /// Context packet budget to simulate (default 7168 bytes).
    #[arg(long, default_value_t = 7168)]
    pub max_bytes: u32,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
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

pub fn run(
    conn: &mut Connection,
    project: i64,
    cwd: &Path,
    root: &Path,
    options: Options,
) -> Result<u8> {
    if let Some(id) = options.id {
        explain_record(conn, project, cwd, id, options.max_bytes, options.json)?;
    } else if let Some(target_path) = &options.path {
        explain_path(conn, project, root, target_path, options.json)?;
    } else {
        explain_query(
            conn,
            project,
            cwd,
            options.query.as_deref(),
            options.max_bytes,
            options.json,
        )?;
    }
    Ok(0)
}

fn explain_record(
    conn: &mut Connection,
    project: i64,
    cwd: &Path,
    id: i64,
    max_bytes: u32,
    json: bool,
) -> Result<()> {
    let modern = crate::storage::version(conn)? >= 2;
    let provenance = crate::storage::version(conn)? >= 3;

    let (kind, content, created, state, key, expires_at, validity, invalid_reason, source, actor) =
        if provenance {
            conn.query_row(
                "SELECT m.kind, m.content, m.created_at, coalesce(d.state, 'active'), d.memory_key, d.expires_at,
                        coalesce(d.validity, 'valid'), d.invalid_reason, o.source, o.actor
                 FROM memories m
                 LEFT JOIN memory_metadata d ON d.memory_id = m.id
                 LEFT JOIN memory_origins o ON o.memory_id = m.id
                 WHERE m.project_id = ?1 AND m.id = ?2",
                params![project, id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?.unwrap_or_else(|| "unknown".into()),
                        r.get::<_, Option<String>>(9)?.unwrap_or_else(|| "unknown".into()),
                    ))
                },
            ).optional()?.context("Memory record not found in this project")?
        } else if modern {
            conn.query_row(
                "SELECT m.kind, m.content, m.created_at, coalesce(d.state, 'active'), d.memory_key, d.expires_at,
                        'valid', NULL, 'manual', 'user'
                 FROM memories m
                 LEFT JOIN memory_metadata d ON d.memory_id = m.id
                 WHERE m.project_id = ?1 AND m.id = ?2",
                params![project, id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, String>(8)?,
                        r.get::<_, String>(9)?,
                    ))
                },
            ).optional()?.context("Memory record not found in this project")?
        } else {
            conn.query_row(
                "SELECT m.kind, m.content, m.created_at, 'active', NULL, NULL, 'valid', NULL, 'manual', 'user'
                 FROM memories m
                 WHERE m.project_id = ?1 AND m.id = ?2",
                params![project, id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, String>(8)?,
                        r.get::<_, String>(9)?,
                    ))
                },
            ).optional()?.context("Memory record not found in this project")?
        };

    let now = crate::memory::now()?;
    let is_stale = expires_at.is_some_and(|e| e <= now);

    // Check key conflict
    let conflict_count: i64 = if let Some(k) = &key {
        conn.query_row(
            "SELECT count(*) FROM memory_metadata d JOIN memories m ON m.id = d.memory_id
             WHERE m.project_id = ?1 AND d.memory_key = ?2 AND coalesce(d.state, 'active') = 'active' AND coalesce(d.validity, 'valid') = 'valid'",
            params![project, k],
            |r| r.get(0),
        ).unwrap_or(0)
    } else {
        0
    };
    let is_conflicted = conflict_count > 1;

    let is_unknown = source == "legacy-unknown" || actor == "unknown";
    let is_sensitive = [
        &kind,
        &content,
        &created,
        &source,
        &actor,
        key.as_deref().unwrap_or(""),
    ]
    .iter()
    .any(|s| sensitive::guard(s, false).is_err());

    // Run context simulation to verify actual packet inclusion
    let request = crate::context::Request {
        protocol_version: 1,
        query: None,
        max_bytes,
        limit: 100,
        include_stale: false,
        include_conflicts: false,
        allow_unknown_provenance: false,
    };

    let (delivered, omission_reason, suggested_fix) = if state != "active" {
        (
            false,
            format!("Inactive state: '{state}'"),
            "Record is archived or superseded. Reactivate or create a replacement if needed.",
        )
    } else if validity != "valid" {
        (
            false,
            format!(
                "Invalidated: '{}'",
                invalid_reason.as_deref().unwrap_or("unknown reason")
            ),
            "Record was marked invalid. Use 'swapctx memory add' to record updated knowledge.",
        )
    } else if is_stale {
        (false, "Expired (stale)".into(), "Record exceeded its expiration date. Renew with 'swapctx memory review <id> --expires-in-days <days>' or make permanent with '--clear-expiry'.")
    } else if is_conflicted {
        (false, format!("Conflicted explicit key \"{}\" (competing with {} records)", key.as_deref().unwrap_or(""), conflict_count), "Competing statements share this key. Resolve by superseding or invalidating the obsolete version.")
    } else if is_unknown {
        (false, "Unknown provenance (legacy origin)".into(), "Record origin is unverified. Re-add or supersede it with explicit --source and --actor.")
    } else if is_sensitive {
        (false, "Sensitive pattern detected".into(), "Secret markers found in content. Invalidate or supersede the record without sensitive credentials.")
    } else {
        // Test packet inclusion via structural lookup inside the records array
        let in_packet = if let Ok(json) = crate::context::packet(conn, cwd, &request) {
            conn.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM json_each(?1, '$.records')
                     WHERE json_extract(value, '$.id') = CAST(?2 AS TEXT)
                        OR json_extract(value, '$.id') = ?2
                 )",
                params![&json, id],
                |r| r.get(0),
            )
            .unwrap_or(false)
        } else {
            false
        };

        if in_packet {
            (
                true,
                "Included in native context packet".into(),
                "No action needed. Record meets all criteria and is included in the context packet (model receipt unverified).",
            )
        } else {
            (false, "Budget or limit overflow".into(), "The record was displaced by earlier requirements or packet budget exhaustion. Increase packet budget or prune obsolete records.")
        }
    };

    if json {
        let output: String = conn.query_row(
            "SELECT json_object(
                'protocol', 'swapctx.explain.record',
                'id', ?1,
                'kind', ?2,
                'delivered', json(?3),
                'omission_reason', ?4,
                'suggested_fix', ?5,
                'metadata', json_object(
                    'state', ?6,
                    'validity', ?7,
                    'stale', json(?8),
                    'conflicted', json(?9),
                    'unknown_provenance', json(?10),
                    'sensitive', json(?11)
                ),
                'model_adherence', 'unverified'
            )",
            params![
                id,
                sensitive::display(&kind, false),
                if delivered { "true" } else { "false" },
                sensitive::display(&omission_reason, false),
                suggested_fix,
                state,
                validity,
                if is_stale { "true" } else { "false" },
                if is_conflicted { "true" } else { "false" },
                if is_unknown { "true" } else { "false" },
                if is_sensitive { "true" } else { "false" }
            ],
            |r| r.get(0),
        )?;
        println!("{output}");
        return Ok(());
    }

    println!("Context Delivery Explanation — Record #{id}\n");
    println!("Kind:       {}", sensitive::display(&kind, false));
    println!(
        "Key:        {}",
        key.as_deref()
            .map(|k| sensitive::display(k, false))
            .unwrap_or_else(|| "none".into())
    );
    println!(
        "State:      {} (validity: {})",
        safe(&state),
        safe(&validity)
    );
    println!(
        "Provenance: source={} actor={}",
        sensitive::display(&source, false),
        sensitive::display(&actor, false)
    );
    println!(
        "Delivery:   {}\n",
        if delivered {
            "DELIVERED IN CONTEXT (local packet inclusion; model receipt unverified)"
        } else {
            "OMITTED FROM CONTEXT"
        }
    );

    println!("Analysis:");
    if delivered {
        println!("  - Record is active, valid, and meets all fresh/unconflicted criteria.");
        println!("  - Included in native context packet within the {max_bytes}-byte budget.");
    } else {
        println!(
            "  - Reason omitted: {}",
            sensitive::display(&omission_reason, false)
        );
        println!("  - Suggested fix:  {}", safe(suggested_fix));
    }

    Ok(())
}

fn explain_path(
    conn: &Connection,
    project: i64,
    _root: &Path,
    target_path: &str,
    json: bool,
) -> Result<()> {
    let norm_path = target_path.replace('\\', "/");

    // Boundary rules matching path
    let mut stmt = conn.prepare(
        "SELECT r.id, r.pattern, r.reason, coalesce(p.policy, 'deny')
         FROM rules r
         LEFT JOIN rule_policies p ON p.rule_id = r.id
         WHERE r.project_id = ?1
         ORDER BY r.id ASC",
    )?;
    let mut matched_rules = Vec::new();
    let mut rows = stmt.query(params![project])?;
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        let pattern: String = r.get(1)?;
        let reason: String = r.get(2)?;
        let policy: String = r.get(3)?;
        if let Ok(matcher) = paths::rule_matcher(&pattern) {
            if matcher.is_match(&norm_path) {
                matched_rules.push((id, pattern, policy, reason));
            }
        }
    }

    // Match requirements whose content or key matches path components
    let components = norm_path
        .split('/')
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>();

    let mut relevant_reqs = Vec::new();
    let mut req_stmt = conn.prepare(
        "SELECT m.id, m.kind, m.content, d.memory_key
         FROM memories m
         LEFT JOIN memory_metadata d ON d.memory_id = m.id
         WHERE m.project_id = ?1
           AND coalesce(d.state, 'active') = 'active'
           AND coalesce(d.validity, 'valid') = 'valid'
           AND m.kind IN ('requirement', 'intent')
         ORDER BY m.id ASC",
    )?;
    let mut req_rows = req_stmt.query(params![project])?;
    while let Some(r) = req_rows.next()? {
        let id: i64 = r.get(0)?;
        let kind: String = r.get(1)?;
        let content: String = r.get(2)?;
        let key: Option<String> = r.get(3)?;

        let text_lower = format!(
            "{} {}",
            content.to_ascii_lowercase(),
            key.as_deref().unwrap_or("").to_ascii_lowercase()
        );
        let matches = components.iter().any(|comp| {
            let stem = comp.split('.').next().unwrap_or(comp).to_ascii_lowercase();
            stem.len() >= 3 && text_lower.contains(&stem)
        });

        if matches {
            relevant_reqs.push((id, kind, content, key));
        }
    }

    if json {
        let rules_json = matched_rules
            .iter()
            .map(|(id, pat, pol, r)| {
                format!(
                    "{{\"id\":{},\"pattern\":\"{}\",\"policy\":\"{}\",\"reason\":\"{}\"}}",
                    id,
                    json_escape(pat),
                    json_escape(pol),
                    json_escape(r)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let reqs_json = relevant_reqs
            .iter()
            .map(|(id, kind, content, key)| {
                format!(
                    "{{\"id\":{},\"kind\":\"{}\",\"content\":\"{}\",\"key\":\"{}\"}}",
                    id,
                    json_escape(kind),
                    json_escape(content),
                    json_escape(key.as_deref().unwrap_or(""))
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let output: String = conn.query_row(
            "SELECT json_object(
                'protocol', 'swapctx.explain.path',
                'path', ?1,
                'matched_rules', json(?2),
                'relevant_requirements', json(?3)
            )",
            params![
                norm_path,
                format!("[{rules_json}]"),
                format!("[{reqs_json}]")
            ],
            |r| r.get(0),
        )?;
        println!("{output}");
        return Ok(());
    }

    println!("Path Context Lens — {}\n", safe(&norm_path));
    println!(
        "Governing Boundary Rules ({} matched):",
        matched_rules.len()
    );
    if matched_rules.is_empty() {
        println!("  (No boundary protection rules match this path)");
    } else {
        for (id, pattern, policy, reason) in &matched_rules {
            println!(
                "  - Rule #{id} [{}]: pattern=\"{}\" reason=\"{}\"",
                policy.to_ascii_uppercase(),
                safe(pattern),
                safe(reason)
            );
        }
    }

    println!(
        "\nRelevant Project Requirements & Intent ({} found):",
        relevant_reqs.len()
    );
    if relevant_reqs.is_empty() {
        println!("  (No specific requirements directly correlate with this path)");
    } else {
        for (id, kind, content, key) in &relevant_reqs {
            let key_str = key
                .as_deref()
                .map(|k| format!(" key=\"{}\"", sensitive::display(k, false)))
                .unwrap_or_default();
            println!("  - #{id} [{}]{}", sensitive::display(kind, false), key_str);
            println!("      {}", sensitive::display(content, false));
        }
    }

    Ok(())
}

fn explain_query(
    conn: &mut Connection,
    _project: i64,
    cwd: &Path,
    query: Option<&str>,
    max_bytes: u32,
    json: bool,
) -> Result<()> {
    let request = crate::context::Request {
        protocol_version: 1,
        query: query.map(String::from),
        max_bytes,
        limit: 20,
        include_stale: false,
        include_conflicts: false,
        allow_unknown_provenance: false,
    };

    let raw_packet = crate::context::packet(conn, cwd, &request)?;

    if json {
        println!("{raw_packet}");
        return Ok(());
    }

    let (
        req_returned,
        req_total,
        rules_returned,
        rules_total,
        bytes_used,
        mandatory_complete,
        omitted_stale,
        omitted_conflicts,
        omitted_budget,
    ): (i64, i64, i64, i64, i64, bool, i64, i64, i64) = conn.query_row(
        "SELECT
                coalesce(json_extract(?1, '$.coverage.requirements_returned'), 0),
                coalesce(json_extract(?1, '$.coverage.active_valid_requirements'), 0),
                coalesce(json_extract(?1, '$.coverage.boundary_rules_returned'), 0),
                coalesce(json_extract(?1, '$.coverage.boundary_rules_total'), 0),
                coalesce(json_extract(?1, '$.budget.bytes_used'), 0),
                coalesce(json_extract(?1, '$.coverage.mandatory_context_complete'), 0),
                coalesce(json_extract(?1, '$.omitted.stale'), 0),
                coalesce(json_extract(?1, '$.omitted.conflicted'), 0),
                coalesce(json_extract(?1, '$.omitted.budget'), 0)",
        [&raw_packet],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
            ))
        },
    )?;

    println!("Context Packet Retrieval Simulation\n");
    if let Some(q) = query {
        println!("Query terms:       \"{}\"", sensitive::display(q, false));
    }
    println!("Packet budget:     {bytes_used} / {max_bytes} bytes");
    println!("Requirements:      {req_returned} / {req_total} delivered");
    println!("Boundary rules:    {rules_returned} / {rules_total} delivered");
    println!(
        "Mandatory status:  {}\n",
        if mandatory_complete {
            "COMPLETE (prepared locally; provider delivery unverified)"
        } else {
            "INCOMPLETE (withheld)"
        }
    );

    println!("Omissions Breakdown:");
    println!("  - Stale / Expired:    {omitted_stale}");
    println!("  - Key conflicts:      {omitted_conflicts}");
    println!("  - Budget overflows:   {omitted_budget}");

    Ok(())
}
