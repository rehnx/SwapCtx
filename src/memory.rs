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
