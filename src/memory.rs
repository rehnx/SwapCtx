use crate::{display::safe, escape_like, history, sensitive, storage, validate_text};
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Subcommand, Debug)]
pub enum Action {
    /// Save explicit context; optional keys identify competing statements.
    Add {
        #[arg(long, default_value = "note")]
        kind: String,
        text: String,
        #[arg(long)]
        key: Option<String>,
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=3650))]
        expires_in_days: Option<u32>,
        #[arg(long)]
        allow_sensitive: bool,
        /// Self-reported origin label; not independently verified.
        #[arg(long, default_value = "manual")]
        source: String,
        #[arg(long, default_value = "user")]
        actor: String,
    },
    /// Literal substring search in active memories; stale entries remain visible.
    Search {
        query: String,
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
        #[arg(long)]
        include_inactive: bool,
        #[arg(long)]
        fresh_only: bool,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        key: Option<String>,
        /// Exclude records whose explicit key has competing valid active records.
        #[arg(long)]
        unconflicted: bool,
    },
    /// List newest active memories; optionally include archived/superseded history.
    Recent {
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
        #[arg(long)]
        include_inactive: bool,
        #[arg(long)]
        fresh_only: bool,
    },
    /// Inspect one record, including its lifecycle metadata.
    Get {
        id: i64,
        /// Explicitly display potentially sensitive saved text.
        #[arg(long)]
        reveal: bool,
    },
    /// Retain a record in history while excluding it from default retrieval.
    Archive { id: i64 },
    /// Mark knowledge invalid, preserving the record and a reason in history.
    Invalidate {
        id: i64,
        #[arg(long)]
        reason: String,
    },
    /// Show self-reported origin and retained local lifecycle history.
    History { id: i64 },
    /// Atomically replace an active record, retaining the original and its link.
    Supersede {
        id: i64,
        text: String,
        #[arg(long, conflicts_with = "clear_expiry", value_parser = clap::value_parser!(u32).range(1..=3650))]
        expires_in_days: Option<u32>,
        #[arg(long)]
        clear_expiry: bool,
        #[arg(long)]
        allow_sensitive: bool,
    },
    /// Record human review, optionally renew or explicitly clear the expiry.
    Review {
        id: i64,
        #[arg(long, conflicts_with = "clear_expiry", value_parser = clap::value_parser!(u32).range(1..=3650))]
        expires_in_days: Option<u32>,
        #[arg(long)]
        clear_expiry: bool,
    },
    /// Health diagnostics and native context readiness for current project memory.
    Audit {
        /// Optional JSON output mode.
        #[arg(long)]
        json: bool,
    },
}

impl Action {
    pub fn writable(&self) -> bool {
        matches!(
            self,
            Self::Add { .. }
                | Self::Archive { .. }
                | Self::Supersede { .. }
                | Self::Review { .. }
                | Self::Invalidate { .. }
        )
    }
}

pub fn now() -> Result<i64> {
    Ok(i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System clock predates Unix epoch")?
            .as_secs(),
    )?)
}

fn expiry(days: Option<u32>) -> Result<Option<i64>> {
    days.map(|d| Ok(now()? + i64::from(d) * 86_400)).transpose()
}

fn check_input(kind: &str, text: &str, key: Option<&str>, allow: bool) -> Result<()> {
    validate_text("Memory kind", kind, 128)?;
    validate_text("Memory content", text, 1024 * 1024)?;
    sensitive::guard(kind, allow)?;
    sensitive::guard(text, allow)?;
    if let Some(key) = key {
        validate_text("Memory key", key, 128)?;
        sensitive::guard(key, allow)?;
    }
    Ok(())
}

fn insert(
    conn: &Connection,
    project: i64,
    kind: &str,
    text: &str,
    key: Option<&str>,
    expires: Option<i64>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO memories(project_id, kind, content) VALUES (?1, ?2, ?3)",
        params![project, kind, text],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute("INSERT INTO memory_metadata(memory_id,memory_key,expires_at,reviewed_at) VALUES (?1,?2,?3,?4)", params![id, key, expires, now()?])?;
    Ok(id)
}

fn active(
    conn: &Connection,
    project: i64,
    id: i64,
) -> Result<(String, Option<String>, Option<i64>)> {
    let (kind, key, expires, state) = conn
        .query_row(
            "SELECT m.kind,d.memory_key,d.expires_at,coalesce(d.state,'active')
         FROM memories m LEFT JOIN memory_metadata d ON d.memory_id=m.id
         WHERE m.project_id=?1 AND m.id=?2",
            params![project, id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .context("Memory record not found in this project")?;
    if state != "active" {
        bail!("Only active memories may be reviewed, archived, or superseded");
    }
    Ok((kind, key, expires))
}

pub fn run(conn: &mut Connection, project: i64, action: Action) -> Result<()> {
    match action {
        Action::Add {
            kind,
            text,
            key,
            expires_in_days,
            allow_sensitive,
            source,
            actor,
        } => {
            check_input(&kind, &text, key.as_deref(), allow_sensitive)?;
            for value in [&source, &actor] {
                validate_text("Provenance label", value, 4096)?;
                sensitive::guard(value, allow_sensitive)?;
            }
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let id = insert(
                &tx,
                project,
                &kind,
                &text,
                key.as_deref(),
                expiry(expires_in_days)?,
            )?;
            history::origin(&tx, id, &source, &actor)?;
            history::event(
                &tx,
                id,
                "add",
                "Explicitly saved by CLI; source and actor are self-reported",
            )?;
            tx.commit()?;
            println!("Saved local memory record #{id}");
        }
        Action::Supersede {
            id,
            text,
            expires_in_days,
            clear_expiry,
            allow_sensitive,
        } => {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let (kind, key, old_expiry) = active(&tx, project, id)?;
            check_input(&kind, &text, key.as_deref(), allow_sensitive)?;
            let replacement = insert(
                &tx,
                project,
                &kind,
                &text,
                key.as_deref(),
                if clear_expiry {
                    None
                } else if expires_in_days.is_some() {
                    expiry(expires_in_days)?
                } else {
                    old_expiry
                },
            )?;
            history::origin(&tx, replacement, "manual-supersession", "user")?;
            history::event(
                &tx,
                replacement,
                "add",
                &format!("Replacement of origin {}", history::identifier(&tx, id)?),
            )?;
            tx.execute("INSERT INTO memory_metadata(memory_id,state,superseded_by) VALUES (?1,'superseded',?2)
                        ON CONFLICT(memory_id) DO UPDATE SET state='superseded', superseded_by=excluded.superseded_by", params![id,replacement])?;
            history::event(
                &tx,
                id,
                "supersede",
                &format!(
                    "Replaced by origin {}",
                    history::identifier(&tx, replacement)?
                ),
            )?;
            tx.commit()?;
            println!("Superseded memory #{id} with #{replacement}; original content retained");
        }
        Action::Archive { id } => {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            active(&tx, project, id)?;
            tx.execute(
                "INSERT INTO memory_metadata(memory_id,state) VALUES (?1,'archived')
                        ON CONFLICT(memory_id) DO UPDATE SET state='archived'",
                [id],
            )?;
            history::event(
                &tx,
                id,
                "archive",
                "Excluded from default retrieval; content retained",
            )?;
            tx.commit()?;
            println!("Archived memory #{id}; content retained");
        }
        Action::Review {
            id,
            expires_in_days,
            clear_expiry,
        } => {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let (_, _, old_expiry) = active(&tx, project, id)?;
            let expires = if clear_expiry {
                None
            } else if expires_in_days.is_some() {
                expiry(expires_in_days)?
            } else {
                old_expiry
            };
            tx.execute("INSERT INTO memory_metadata(memory_id,reviewed_at,expires_at) VALUES (?1,?2,?3)
                        ON CONFLICT(memory_id) DO UPDATE SET reviewed_at=excluded.reviewed_at,expires_at=excluded.expires_at", params![id,now()?,expires])?;
            history::event(&tx,id,"review",&format!("Human review; expiry before={old_expiry:?} after={expires:?}; no correctness inferred"))?;
            tx.commit()?;
            println!("Recorded human review for memory #{id}; no factual correctness inferred");
        }
        Action::History { id } => history::show(conn, project, id)?,
        Action::Invalidate { id, reason } => {
            validate_text("Invalidation reason", &reason, 16384)?;
            sensitive::guard(&reason, false)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            active(&tx, project, id)?;
            tx.execute("INSERT INTO memory_metadata(memory_id,validity,invalid_reason) VALUES(?1,'invalid',?2) ON CONFLICT(memory_id) DO UPDATE SET validity='invalid',invalid_reason=excluded.invalid_reason",params![id,reason])?;
            history::event(&tx, id, "invalidate", &reason)?;
            tx.commit()?;
            println!("Invalidated memory #{id}; original data and reason retained");
        }
        Action::Search {
            query,
            limit,
            include_inactive,
            fresh_only,
            kind,
            key,
            unconflicted,
        } => {
            validate_text("Search query", &query, 4096)?;
            if let Some(value) = &kind {
                validate_text("Kind filter", value, 128)?;
            }
            if let Some(value) = &key {
                validate_text("Key filter", value, 128)?;
            }
            retrieve(
                conn,
                project,
                Retrieval {
                    query: Some(&query),
                    limit,
                    include_inactive,
                    fresh_only,
                    kind: kind.as_deref(),
                    key: key.as_deref(),
                    unconflicted,
                    ..Default::default()
                },
            )?;
        }
        Action::Recent {
            limit,
            include_inactive,
            fresh_only,
        } => retrieve(
            conn,
            project,
            Retrieval {
                limit,
                include_inactive,
                fresh_only,
                ..Default::default()
            },
        )?,
        Action::Get { id, reveal } => retrieve(
            conn,
            project,
            Retrieval {
                id: Some(id),
                limit: 1,
                include_inactive: true,
                reveal,
                ..Default::default()
            },
        )?,
        Action::Audit { json } => audit(conn, project, json)?,
    }
    Ok(())
}

#[derive(Default)]
struct Retrieval<'a> {
    query: Option<&'a str>,
    id: Option<i64>,
    limit: u32,
    include_inactive: bool,
    fresh_only: bool,
    reveal: bool,
    kind: Option<&'a str>,
    key: Option<&'a str>,
    unconflicted: bool,
}

// Legacy stores remain readable without a migration or write-capable connection.
fn retrieve(conn: &Connection, project: i64, filter: Retrieval<'_>) -> Result<()> {
    let Retrieval {
        query,
        id,
        limit,
        include_inactive,
        fresh_only,
        reveal,
        kind: kind_filter,
        key: key_filter,
        unconflicted,
    } = filter;
    let modern = storage::version(conn)? >= 2;
    let provenance = storage::version(conn)? >= 3;
    let validity = if provenance {
        "coalesce(d.validity,'valid'),d.invalid_reason"
    } else {
        "'valid',NULL"
    };
    let valid = if provenance && !include_inactive {
        "AND coalesce(d.validity,'valid')='valid'"
    } else {
        ""
    };
    let key_clause = if modern {
        "AND (?7 IS NULL OR d.memory_key=?7)"
    } else {
        "AND ?7 IS NULL"
    };
    let conflict_clause = if modern && unconflicted {
        if provenance {
            "AND (d.memory_key IS NULL OR d.memory_key NOT IN (SELECT c.memory_key FROM memory_metadata c JOIN memories cm ON cm.id=c.memory_id WHERE cm.project_id=?1 AND c.memory_key IS NOT NULL AND c.state='active' AND c.validity='valid' GROUP BY c.memory_key HAVING count(*)>1))"
        } else {
            "AND (d.memory_key IS NULL OR d.memory_key NOT IN (SELECT c.memory_key FROM memory_metadata c JOIN memories cm ON cm.id=c.memory_id WHERE cm.project_id=?1 AND c.memory_key IS NOT NULL AND c.state='active' GROUP BY c.memory_key HAVING count(*)>1))"
        }
    } else {
        ""
    };
    let metadata = if modern {
        "coalesce(d.state,'active'),d.memory_key,d.expires_at,d.reviewed_at,d.superseded_by"
    } else {
        "'active',NULL,NULL,NULL,NULL"
    };
    let join = if modern {
        "LEFT JOIN memory_metadata d ON d.memory_id=m.id"
    } else {
        ""
    };
    let active = if modern && !include_inactive {
        "AND coalesce(d.state,'active')='active'"
    } else {
        ""
    };
    let fresh = if modern && fresh_only {
        "AND (d.expires_at IS NULL OR d.expires_at > ?5)"
    } else {
        "AND ?5 IS NOT NULL"
    };
    let search = if query.is_some() {
        "AND (m.content LIKE ?2 ESCAPE '\\' OR m.kind LIKE ?2 ESCAPE '\\')"
    } else {
        "AND ?2 IS NULL"
    };
    let record = if id.is_some() {
        "AND m.id=?3"
    } else {
        "AND ?3 IS NULL"
    };
    let sql = format!(
        "SELECT m.id,m.kind,m.content,m.created_at,{metadata},{validity} FROM memories m {join}
        WHERE m.project_id=?1 {search} {record} {active} {fresh} {valid} AND (?6 IS NULL OR m.kind=?6) {key_clause} {conflict_clause} ORDER BY m.id DESC LIMIT ?4"
    );
    let like = query.map(|q| format!("%{}%", escape_like(q)));
    let timestamp = now()?;
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![
        project,
        like,
        id,
        limit,
        timestamp,
        kind_filter,
        key_filter
    ])?;
    let mut count = 0;
    let mut conflicts = std::collections::HashMap::new();
    while let Some(row) = rows.next()? {
        let record: i64 = row.get(0)?;
        let kind: String = row.get(1)?;
        let content: String = row.get(2)?;
        let created: String = row.get(3)?;
        let state: String = row.get(4)?;
        let key: Option<String> = row.get(5)?;
        let expires: Option<i64> = row.get(6)?;
        let reviewed: Option<i64> = row.get(7)?;
        let replacement: Option<i64> = row.get(8)?;
        let validity: String = row.get(9)?;
        let invalid_reason: Option<String> = row.get(10)?;
        let stale = expires.is_some_and(|e| e <= timestamp);
        println!(
            "#{record} [{}] {} state={}{}",
            sensitive::display(&kind, reveal),
            safe(&created),
            safe(&state),
            if stale { " STALE" } else { "" }
        );
        if validity == "invalid" {
            println!(
                "  INVALID: {}",
                sensitive::display(
                    invalid_reason.as_deref().unwrap_or("unknown reason"),
                    reveal
                )
            );
        }
        if let Some(key) = key {
            let competing: i64 = if let Some(count) = conflicts.get(&key) {
                *count
            } else {
                let count=conn.query_row(
                if provenance { "SELECT count(*) FROM memory_metadata d JOIN memories m ON m.id=d.memory_id WHERE m.project_id=?1 AND d.memory_key=?2 AND d.state='active' AND d.validity='valid'" } else { "SELECT count(*) FROM memory_metadata d JOIN memories m ON m.id=d.memory_id WHERE m.project_id=?1 AND d.memory_key=?2 AND d.state='active'" },
                params![project, key],
                |r| r.get(0),
            )?;
                conflicts.insert(key.clone(), count);
                count
            };
            println!(
                "  key={}{}",
                sensitive::display(&key, reveal),
                if competing > 1 {
                    format!(" CONFLICT: {competing} active records share this explicit key")
                } else {
                    String::new()
                }
            );
        }
        if expires.is_some() || reviewed.is_some() || replacement.is_some() {
            println!("  expires_unix={expires:?} reviewed_unix={reviewed:?} superseded_by={replacement:?}");
        }
        println!("  {}", sensitive::display(&content, reveal));
        count += 1;
    }
    if let Some(id) = id.filter(|_| count > 0) {
        history::show(conn, project, id)?;
    }
    if count == 0 {
        if id.is_some() {
            bail!("Memory record not found in this project");
        }
        println!("No memory records found.");
    }
    Ok(())
}

struct ExpiredRecord {
    id: i64,
    kind: String,
    key: Option<String>,
    is_requirement: bool,
}

struct ConflictingKeyInfo {
    key: String,
    record_ids: Vec<i64>,
}

struct UnknownProvRecord {
    id: i64,
    kind: String,
    key: Option<String>,
    source: String,
    actor: String,
    is_requirement: bool,
}

struct SensitiveRecordInfo {
    id: i64,
    kind: String,
    key: Option<String>,
    is_requirement: bool,
}

struct ContextSim {
    mandatory_complete: bool,
    requirements_returned: i64,
    rules_returned: i64,
    rules_total: i64,
    review_required: i64,
    bytes_used: usize,
    scan_truncated: bool,
    packet_err: Option<String>,
}

/// Exact duplicate candidates only; similarity never silently merges knowledge.
fn duplicate_records(conn: &Connection, project: i64, version: i64) -> Result<Vec<Vec<i64>>> {
    let (join, eligible, key) = if version >= 3 {
        (
            "LEFT JOIN memory_metadata d ON d.memory_id=m.id",
            "AND coalesce(d.state,'active')='active' AND coalesce(d.validity,'valid')='valid'",
            "coalesce(d.memory_key,'')",
        )
    } else if version >= 2 {
        (
            "LEFT JOIN memory_metadata d ON d.memory_id=m.id",
            "AND coalesce(d.state,'active')='active'",
            "coalesce(d.memory_key,'')",
        )
    } else {
        ("", "", "''")
    };
    let sql = format!(
        "SELECT group_concat(m.id ORDER BY m.id) FROM memories m {join}
         WHERE m.project_id=?1 {eligible}
         GROUP BY lower(m.kind), m.content, {key} HAVING count(*) > 1 ORDER BY min(m.id)"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([project])?;
    let mut groups = Vec::new();
    while let Some(row) = rows.next()? {
        let ids: String = row.get(0)?;
        groups.push(
            ids.split(',')
                .map(str::parse)
                .collect::<std::result::Result<Vec<i64>, _>>()?,
        );
    }
    Ok(groups)
}

fn audit(conn: &mut Connection, project: i64, json: bool) -> Result<()> {
    let tx = conn.transaction()?;
    audit_snapshot(&tx, project, json)?;
    tx.commit()?;
    Ok(())
}

fn audit_snapshot(conn: &Connection, project: i64, json: bool) -> Result<()> {
    let version = storage::version(conn)?;
    let modern = version >= 2;
    let provenance = version >= 3;

    let integrity_error = storage::diagnose(conn).err().map(|e| format!("{e:#}"));

    let total_memories: i64 = conn.query_row(
        "SELECT count(*) FROM memories WHERE project_id = ?1",
        [project],
        |r| r.get(0),
    )?;

    let (active_memories, archived_memories, superseded_memories, invalidated_memories) =
        if provenance {
            let active: i64 = conn.query_row(
            "SELECT count(*) FROM memories m LEFT JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND coalesce(d.state, 'active') = 'active' AND coalesce(d.validity, 'valid') = 'valid'",
            [project],
            |r| r.get(0),
        )?;
            let archived: i64 = conn.query_row(
            "SELECT count(*) FROM memories m JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND d.state = 'archived'",
            [project],
            |r| r.get(0),
        )?;
            let superseded: i64 = conn.query_row(
            "SELECT count(*) FROM memories m JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND d.state = 'superseded'",
            [project],
            |r| r.get(0),
        )?;
            let invalidated: i64 = conn.query_row(
            "SELECT count(*) FROM memories m JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND d.validity = 'invalid'",
            [project],
            |r| r.get(0),
        )?;
            (active, archived, superseded, invalidated)
        } else if modern {
            let active: i64 = conn.query_row(
            "SELECT count(*) FROM memories m LEFT JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND coalesce(d.state, 'active') = 'active'",
            [project],
            |r| r.get(0),
        )?;
            let archived: i64 = conn.query_row(
            "SELECT count(*) FROM memories m JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND d.state = 'archived'",
            [project],
            |r| r.get(0),
        )?;
            let superseded: i64 = conn.query_row(
            "SELECT count(*) FROM memories m JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND d.state = 'superseded'",
            [project],
            |r| r.get(0),
        )?;
            (active, archived, superseded, 0)
        } else {
            (total_memories, 0, 0, 0)
        };

    let active_valid_requirements: i64 = if provenance {
        conn.query_row(
            "SELECT count(*) FROM memories m LEFT JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND m.kind = 'requirement' COLLATE NOCASE AND coalesce(d.state, 'active') = 'active' AND coalesce(d.validity, 'valid') = 'valid'",
            [project],
            |r| r.get(0),
        )?
    } else if modern {
        conn.query_row(
            "SELECT count(*) FROM memories m LEFT JOIN memory_metadata d ON d.memory_id = m.id WHERE m.project_id = ?1 AND m.kind = 'requirement' COLLATE NOCASE AND coalesce(d.state, 'active') = 'active'",
            [project],
            |r| r.get(0),
        )?
    } else {
        conn.query_row(
            "SELECT count(*) FROM memories WHERE project_id = ?1 AND kind = 'requirement' COLLATE NOCASE",
            [project],
            |r| r.get(0),
        )?
    };

    let timestamp = now()?;
    let mut expired_records = Vec::new();
    if modern {
        let sql = format!(
            "SELECT m.id, m.kind, d.memory_key, d.expires_at
             FROM memories m JOIN memory_metadata d ON d.memory_id = m.id
             WHERE m.project_id = ?1 AND coalesce(d.state, 'active') = 'active'
               AND d.expires_at IS NOT NULL AND d.expires_at <= ?2
               {}
             ORDER BY m.id ASC",
            if provenance {
                "AND coalesce(d.validity,'valid')='valid'"
            } else {
                ""
            }
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![project, timestamp])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let kind: String = r.get(1)?;
            let key: Option<String> = r.get(2)?;
            let is_requirement = kind.eq_ignore_ascii_case("requirement");
            expired_records.push(ExpiredRecord {
                id,
                kind,
                key,
                is_requirement,
            });
        }
    }
    let expired_count = expired_records.len();
    let expired_requirements_count = expired_records.iter().filter(|r| r.is_requirement).count();

    let mut conflicting_keys = Vec::new();
    let mut conflicting_records_count = 0;
    if modern {
        let sql = if provenance {
            "SELECT d.memory_key
             FROM memory_metadata d JOIN memories m ON m.id = d.memory_id
             WHERE m.project_id = ?1 AND d.memory_key IS NOT NULL AND d.state = 'active' AND d.validity = 'valid'
             GROUP BY d.memory_key HAVING count(*) > 1
             ORDER BY d.memory_key ASC"
        } else {
            "SELECT d.memory_key
             FROM memory_metadata d JOIN memories m ON m.id = d.memory_id
             WHERE m.project_id = ?1 AND d.memory_key IS NOT NULL AND d.state = 'active'
             GROUP BY d.memory_key HAVING count(*) > 1
             ORDER BY d.memory_key ASC"
        };
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(params![project])?;
        let mut keys = Vec::new();
        while let Some(r) = rows.next()? {
            let key: String = r.get(0)?;
            keys.push(key);
        }
        drop(rows);
        drop(stmt);

        for key in keys {
            let rec_sql = if provenance {
                "SELECT m.id
                 FROM memories m JOIN memory_metadata d ON d.memory_id = m.id
                 WHERE m.project_id = ?1 AND d.memory_key = ?2 AND d.state = 'active' AND d.validity = 'valid'
                 ORDER BY m.id ASC"
            } else {
                "SELECT m.id
                 FROM memories m JOIN memory_metadata d ON d.memory_id = m.id
                 WHERE m.project_id = ?1 AND d.memory_key = ?2 AND d.state = 'active'
                 ORDER BY m.id ASC"
            };
            let mut rec_stmt = conn.prepare(rec_sql)?;
            let mut rec_rows = rec_stmt.query(params![project, key])?;
            let mut ids = Vec::new();
            while let Some(rr) = rec_rows.next()? {
                let id: i64 = rr.get(0)?;
                ids.push(id);
            }
            conflicting_records_count += ids.len();
            conflicting_keys.push(ConflictingKeyInfo {
                key,
                record_ids: ids,
            });
        }
    }
    let conflicting_keys_count = conflicting_keys.len();

    let mut unknown_provenance_records = Vec::new();
    if provenance {
        let mut stmt = conn.prepare(
            "SELECT m.id, m.kind, d.memory_key, coalesce(o.source, 'legacy-unknown'), coalesce(o.actor, 'unknown')
             FROM memories m
             LEFT JOIN memory_metadata d ON d.memory_id = m.id
             LEFT JOIN memory_origins o ON o.memory_id = m.id
             WHERE m.project_id = ?1
               AND coalesce(d.state, 'active') = 'active'
               AND coalesce(d.validity, 'valid') = 'valid'
               AND (o.source IS NULL OR o.actor IS NULL OR o.source = 'legacy-unknown' OR o.actor = 'unknown')
             ORDER BY m.id ASC",
        )?;
        let mut rows = stmt.query(params![project])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let kind: String = r.get(1)?;
            let key: Option<String> = r.get(2)?;
            let source: String = r.get(3)?;
            let actor: String = r.get(4)?;
            let is_requirement = kind.eq_ignore_ascii_case("requirement");
            unknown_provenance_records.push(UnknownProvRecord {
                id,
                kind,
                key,
                source,
                actor,
                is_requirement,
            });
        }
    }
    let unknown_provenance_count = unknown_provenance_records.len();
    let unknown_provenance_requirements = unknown_provenance_records
        .iter()
        .filter(|r| r.is_requirement)
        .count();

    let mut sensitive_records = Vec::new();
    {
        let sql = if provenance {
            "SELECT m.id, m.kind, m.content, m.created_at, d.memory_key, coalesce(o.source, 'legacy-unknown'), coalesce(o.actor, 'unknown')
             FROM memories m
             LEFT JOIN memory_metadata d ON d.memory_id = m.id
             LEFT JOIN memory_origins o ON o.memory_id = m.id
             WHERE m.project_id = ?1 AND coalesce(d.state, 'active') = 'active' AND coalesce(d.validity, 'valid') = 'valid'
             ORDER BY m.id ASC"
        } else if modern {
            "SELECT m.id, m.kind, m.content, m.created_at, d.memory_key, 'manual', 'user'
             FROM memories m
             LEFT JOIN memory_metadata d ON d.memory_id = m.id
             WHERE m.project_id = ?1 AND coalesce(d.state, 'active') = 'active'
             ORDER BY m.id ASC"
        } else {
            "SELECT id, kind, content, created_at, NULL, 'legacy-unknown', 'unknown'
             FROM memories WHERE project_id = ?1 ORDER BY id ASC"
        };
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(params![project])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let kind: String = r.get(1)?;
            let content: String = r.get(2)?;
            let created: String = r.get(3)?;
            let key: Option<String> = r.get(4)?;
            let source: String = r.get(5)?;
            let actor: String = r.get(6)?;
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
            if is_sensitive {
                let is_requirement = kind.eq_ignore_ascii_case("requirement");
                sensitive_records.push(SensitiveRecordInfo {
                    id,
                    kind,
                    key,
                    is_requirement,
                });
            }
        }
    }
    let sensitive_count = sensitive_records.len();
    let sensitive_requirements_count = sensitive_records
        .iter()
        .filter(|r| r.is_requirement)
        .count();

    let duplicates = duplicate_records(conn, project, version)?;
    let duplicate_records_count: usize = duplicates.iter().map(Vec::len).sum();
    let mut review_recommended_ids = std::collections::HashSet::new();
    for group in &duplicates {
        review_recommended_ids.extend(group.iter().copied());
    }
    for rec in &expired_records {
        review_recommended_ids.insert(rec.id);
    }
    for conf in &conflicting_keys {
        for id in &conf.record_ids {
            review_recommended_ids.insert(*id);
        }
    }
    for prov in &unknown_provenance_records {
        review_recommended_ids.insert(prov.id);
    }
    for record in &sensitive_records {
        review_recommended_ids.insert(record.id);
    }
    let review_recommended_count = review_recommended_ids.len();

    let total_rules: i64 = conn.query_row(
        "SELECT count(*) FROM rules WHERE project_id = ?1",
        [project],
        |r| r.get(0),
    )?;

    let mut invalid_rules = Vec::new();
    let mut sensitive_rules = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, pattern, reason FROM rules WHERE project_id = ?1 ORDER BY id ASC",
        )?;
        let mut rows = stmt.query(params![project])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let pattern: String = r.get(1)?;
            let reason: String = r.get(2)?;
            if let Err(e) = crate::paths::rule_matcher(&pattern) {
                invalid_rules.push((id, pattern.clone(), format!("{e:#}")));
            }
            if sensitive::guard(&pattern, false).is_err()
                || sensitive::guard(&reason, false).is_err()
            {
                sensitive_rules.push((id, pattern));
            }
        }
    }

    let sim = if provenance {
        let root_str: String =
            conn.query_row("SELECT path FROM projects WHERE id=?1", [project], |r| {
                r.get(0)
            })?;
        let cwd = std::path::PathBuf::from(root_str);
        let request = crate::context::Request {
            protocol_version: 1,
            query: None,
            max_bytes: 7168,
            limit: 100,
            include_stale: false,
            include_conflicts: false,
            allow_unknown_provenance: false,
        };
        match crate::context::packet_in_snapshot(conn, &cwd, &request, timestamp) {
            Ok(raw_json) => {
                let bytes_used = raw_json.trim_end().len();
                let (
                    mandatory_complete,
                    req_returned,
                    rules_returned,
                    rules_total,
                    review_required,
                    scan_truncated,
                ): (bool, i64, i64, i64, i64, bool) = conn.query_row(
                    "SELECT
                        coalesce(json_extract(?1, '$.coverage.mandatory_context_complete'), 0),
                        coalesce(json_extract(?1, '$.coverage.requirements_returned'), 0),
                        coalesce(json_extract(?1, '$.coverage.boundary_rules_returned'), 0),
                        coalesce(json_extract(?1, '$.coverage.boundary_rules_total'), 0),
                        coalesce(json_extract(?1, '$.coverage.review_required_records'), 0),
                        coalesce(json_extract(?1, '$.scan.truncated'), 0)",
                    [&raw_json],
                    |r| {
                        Ok((
                            r.get::<_, bool>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, bool>(5)?,
                        ))
                    },
                )?;
                ContextSim {
                    mandatory_complete,
                    requirements_returned: req_returned,
                    rules_returned,
                    rules_total,
                    review_required,
                    bytes_used,
                    scan_truncated,
                    packet_err: None,
                }
            }
            Err(e) => ContextSim {
                mandatory_complete: false,
                requirements_returned: 0,
                rules_returned: 0,
                rules_total: total_rules,
                review_required: 0,
                bytes_used: 0,
                scan_truncated: false,
                packet_err: Some(format!("{e:#}")),
            },
        }
    } else {
        ContextSim {
            mandatory_complete: false,
            requirements_returned: 0,
            rules_returned: 0,
            rules_total: total_rules,
            review_required: 0,
            bytes_used: 0,
            scan_truncated: false,
            packet_err: Some(format!(
                "Database schema is version {version}; native context requires schema version 3"
            )),
        }
    };

    let is_blocked = integrity_error.is_some()
        || version < 3
        || !invalid_rules.is_empty()
        || sim.packet_err.is_some();

    let needs_attention = is_blocked
        || !sim.mandatory_complete
        || sim.requirements_returned < active_valid_requirements
        || sim.rules_returned < total_rules
        || sim.review_required > 0
        || sim.scan_truncated
        || conflicting_keys_count > 0
        || expired_count > 0
        || unknown_provenance_count > 0
        || sensitive_count > 0
        || !sensitive_rules.is_empty()
        || !duplicates.is_empty();

    let readiness = if is_blocked {
        "BLOCKED"
    } else if needs_attention {
        "NEEDS ATTENTION"
    } else {
        "READY"
    };

    let mut suggestions = Vec::new();
    if let Some(err) = &integrity_error {
        suggestions.push(format!(
            "Database integrity failure: {err}. Preserve store and restore a valid backup."
        ));
    }
    if version < 3 {
        suggestions.push(
            "Upgrade database schema to version 3 using a write command or restore to enable native context."
                .into(),
        );
    }
    for (id, pattern, err) in &invalid_rules {
        suggestions.push(format!(
            "Boundary rule #{id} with pattern '{}' is invalid ({}). Update or remove the rule.",
            sensitive::display(pattern, false),
            sensitive::display(err, false)
        ));
    }
    for (id, pattern) in &sensitive_rules {
        suggestions.push(format!(
            "Boundary rule #{id} with pattern '{}' contains sensitive markers. Update the rule to remove secret text.",
            sensitive::display(pattern, false)
        ));
    }
    for group in &duplicates {
        let ids = group
            .iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(", ");
        suggestions.push(format!("Review exact duplicate candidates ({ids}). Compare provenance with 'swapctx memory get <id>' and archive redundant records deliberately; no records were merged."));
    }
    for conf in &conflicting_keys {
        let ids_str = conf
            .record_ids
            .iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(", ");
        suggestions.push(format!(
            "Review the conflicting records associated with key \"{}\" (records {}).",
            sensitive::display(&conf.key, false),
            ids_str
        ));
    }
    for exp in expired_records.iter().filter(|r| r.is_requirement) {
        suggestions.push(format!(
            "Active requirement #{} is expired. Use 'swapctx memory review {} --expires-in-days <days>' to extend validity, '--clear-expiry' to make permanent, or 'swapctx memory archive {}' if obsolete.",
            exp.id, exp.id, exp.id
        ));
    }
    for prov in &unknown_provenance_records {
        suggestions.push(format!(
            "Active {} #{} has unknown provenance ({}/{}). Re-add or supersede it with explicit --source and --actor.",
            if prov.is_requirement { "requirement" } else { "record" },
            prov.id,
            sensitive::display(&prov.source, false),
            sensitive::display(&prov.actor, false)
        ));
    }
    for sens in &sensitive_records {
        suggestions.push(format!(
            "Active record #{} contains sensitive secret markers and is excluded from native context. Supersede or re-add it without secret text.",
            sens.id
        ));
    }
    for exp in expired_records.iter().filter(|r| !r.is_requirement) {
        suggestions.push(format!(
            "Active record #{} ({}) is expired. Review with 'swapctx memory review {}' or archive with 'swapctx memory archive {}'.",
            exp.id,
            sensitive::display(&exp.kind, false),
            exp.id,
            exp.id
        ));
    }
    if sim.scan_truncated {
        suggestions.push(
            "Candidate scan limit (5000) reached. Archive or invalidate obsolete records to reduce active project memory size."
                .into(),
        );
    }
    if sim.packet_err.is_some() {
        suggestions.push("Context simulation failed. Run 'swapctx doctor' and resolve reported integrity or schema problems.".into());
    } else if !sim.mandatory_complete
        || sim.requirements_returned < active_valid_requirements
        || sim.rules_returned < total_rules
    {
        suggestions.push("Mandatory context is incomplete at the 7168-byte budget. Review exclusions and use 'swapctx explain' before sharing context.".into());
    }
    if suggestions.is_empty() {
        if total_memories == 0 {
            suggestions.push(
                "No actions needed. Project memory is empty. Use 'swapctx memory add' to record project context."
                    .into(),
            );
        } else {
            suggestions.push(
                "No actions needed. All active requirements and boundary rules are ready for native context retrieval."
                    .into(),
            );
        }
    }

    let mut exclusions = Vec::new();
    for exp in &expired_records {
        let key_sub = exp
            .key
            .as_deref()
            .map(|k| format!(" key=\"{}\"", sensitive::display(k, false)))
            .unwrap_or_default();
        exclusions.push(format!(
            "#{} [{}]{}: expired (stale)",
            exp.id,
            sensitive::display(&exp.kind, false),
            key_sub
        ));
    }
    for conf in &conflicting_keys {
        for id in &conf.record_ids {
            exclusions.push(format!(
                "#{id}: key conflict \"{}\"",
                sensitive::display(&conf.key, false)
            ));
        }
    }
    for prov in &unknown_provenance_records {
        let key_sub = prov
            .key
            .as_deref()
            .map(|k| format!(" key=\"{}\"", sensitive::display(k, false)))
            .unwrap_or_default();
        exclusions.push(format!(
            "#{} [{}]{}: unknown provenance ({}/{})",
            prov.id,
            sensitive::display(&prov.kind, false),
            key_sub,
            sensitive::display(&prov.source, false),
            sensitive::display(&prov.actor, false)
        ));
    }
    for sens in &sensitive_records {
        let key_sub = sens
            .key
            .as_deref()
            .map(|k| format!(" key=\"{}\"", sensitive::display(k, false)))
            .unwrap_or_default();
        exclusions.push(format!(
            "#{} [{}]{}: sensitive secret markers detected",
            sens.id,
            sensitive::display(&sens.kind, false),
            key_sub
        ));
    }
    exclusions.sort();
    exclusions.dedup();

    if json {
        let actions_json = format!(
            "[{}]",
            suggestions
                .iter()
                .map(|s| format!("\"{}\"", crate::handoff::json_escape(s)))
                .collect::<Vec<_>>()
                .join(",")
        );
        let exclusions_json = format!(
            "[{}]",
            exclusions
                .iter()
                .map(|s| format!("\"{}\"", crate::handoff::json_escape(s)))
                .collect::<Vec<_>>()
                .join(",")
        );
        let output: String = conn.query_row(
            "SELECT json_object(
                'protocol', 'swapctx.memory_audit',
                'version', 1,
                'readiness', ?1,
                'summary', json_object(
                    'active_records', ?2,
                    'valid_requirements', ?3,
                    'expired_records', ?4,
                    'conflicting_keys', ?5,
                    'review_recommended', ?6
                ),
                'inventory', json_object(
                    'total_records', ?7,
                    'active_records', ?8,
                    'valid_requirements', ?9,
                    'archived', ?10,
                    'superseded', ?11,
                    'invalidated', ?12
                ),
                'quality', json_object(
                    'expired_records', ?13,
                    'expired_requirements', ?14,
                    'conflicting_keys', ?15,
                    'conflicting_records', ?16,
                    'unknown_provenance', ?17,
                    'unknown_provenance_requirements', ?18,
                    'sensitive_records', ?19,
                    'sensitive_requirements', ?20,
                    'review_recommended', ?21,
                    'duplicate_groups', ?32,
                    'duplicate_records', ?33,
                    'duplicates', json(?34)
                ),
                'coverage', json_object(
                    'requirements_total', ?22,
                    'requirements_delivered', ?23,
                    'rules_total', ?24,
                    'rules_delivered', ?25,
                    'mandatory_complete', json(?26),
                    'review_required_records', ?27,
                    'bytes_used', ?28,
                    'max_bytes', 7168,
                    'scan_truncated', json(?29)
                ),
                'exclusions', json(?30),
                'suggested_actions', json(?31)
            )",
            params![
                readiness,
                active_memories,
                active_valid_requirements,
                expired_count as i64,
                conflicting_keys_count as i64,
                review_recommended_count as i64,
                total_memories,
                active_memories,
                active_valid_requirements,
                archived_memories,
                superseded_memories,
                invalidated_memories,
                expired_count as i64,
                expired_requirements_count as i64,
                conflicting_keys_count as i64,
                conflicting_records_count as i64,
                unknown_provenance_count as i64,
                unknown_provenance_requirements as i64,
                sensitive_count as i64,
                sensitive_requirements_count as i64,
                review_recommended_count as i64,
                active_valid_requirements,
                sim.requirements_returned,
                total_rules,
                sim.rules_returned,
                if sim.mandatory_complete {
                    "true"
                } else {
                    "false"
                },
                sim.review_required,
                sim.bytes_used as i64,
                if sim.scan_truncated { "true" } else { "false" },
                exclusions_json,
                actions_json,
                duplicates.len() as i64,
                duplicate_records_count as i64,
                format!(
                    "[{}]",
                    duplicates
                        .iter()
                        .map(|group| format!(
                            "[{}]",
                            group
                                .iter()
                                .map(|id| format!("\"{id}\""))
                                .collect::<Vec<_>>()
                                .join(",")
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            ],
            |r| r.get(0),
        )?;
        println!("{output}");
        return Ok(());
    }

    println!("Memory Health — Current Project\n");
    println!("Active records: {active_memories}");
    println!("Valid requirements: {active_valid_requirements}");
    println!("Expired records: {expired_count}");
    println!("Conflicting keys: {conflicting_keys_count}");
    println!("Review recommended: {review_recommended_count}\n");
    println!("Native context readiness: {readiness}\n");

    println!("Inventory & Lifecycle:");
    println!("  Total records: {total_memories}");
    println!("  Active records: {active_memories}");
    println!("  Valid requirements: {active_valid_requirements}");
    println!(
        "  Archived: {archived_memories}, Superseded: {superseded_memories}, Invalidated: {invalidated_memories}\n"
    );

    println!("Quality & Freshness:");
    let exp_sub = if expired_count > 0 {
        format!(" (requirements: {expired_requirements_count})")
    } else {
        String::new()
    };
    println!("  Expired records: {expired_count}{exp_sub}");
    let conf_sub = if conflicting_keys_count > 0 {
        format!(" (competing records: {conflicting_records_count})")
    } else {
        String::new()
    };
    println!("  Conflicting keys: {conflicting_keys_count}{conf_sub}");
    let unk_sub = if unknown_provenance_count > 0 {
        format!(" (requirements: {unknown_provenance_requirements})")
    } else {
        String::new()
    };
    println!("  Unknown provenance: {unknown_provenance_count}{unk_sub}");
    println!(
        "  Duplicate groups: {} (records: {duplicate_records_count})",
        duplicates.len()
    );
    println!("  Review recommended: {review_recommended_count}\n");

    println!("Native Context Coverage:");
    println!(
        "  Mandatory requirements: {}/{} delivered",
        sim.requirements_returned, active_valid_requirements
    );
    println!(
        "  Boundary rules: {}/{} delivered",
        sim.rules_returned, sim.rules_total
    );
    println!("  Review-required records: {}", sim.review_required);
    println!("  Context packet budget: {} / 7168 bytes\n", sim.bytes_used);

    if !exclusions.is_empty() {
        println!("Exclusions from Native Context:");
        for exc in exclusions.iter().take(10) {
            println!("  - {exc}");
        }
        if exclusions.len() > 10 {
            println!("  ... and {} more excluded records", exclusions.len() - 10);
        }
        println!();
    }

    if suggestions.len() == 1 {
        println!("Suggested action:");
        println!("  {}", suggestions[0]);
    } else {
        println!("Suggested actions:");
        for s in &suggestions {
            println!("  - {s}");
        }
    }

    Ok(())
}
