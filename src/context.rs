//! Versioned read-only JSON context packets. SQLite's JSON encoder owns escaping.
use anyhow::{bail, Result};
use clap::Args;
use rusqlite::{params, params_from_iter, types::Value, Connection};
use std::path::Path;

#[derive(Args, Debug)]
pub struct Request {
    #[arg(long, default_value_t = 1)]
    pub protocol_version: u32,
    #[arg(long)]
    pub query: Option<String>,
    /// Complete UTF-8 JSON packet budget, not a model-token estimate.
    #[arg(long, default_value_t=32768, value_parser=clap::value_parser!(u32).range(2048..=131072))]
    pub max_bytes: u32,
    #[arg(long, default_value_t=20, value_parser=clap::value_parser!(u32).range(1..=100))]
    pub limit: u32,
    #[arg(long)]
    pub include_stale: bool,
    #[arg(long)]
    pub include_conflicts: bool,
    #[arg(long)]
    pub allow_unknown_provenance: bool,
}

// Escapes JSON-valid but terminal-active Unicode without changing parsed strings.
fn safe_json(value: String) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_control()
            || matches!(ch,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'|'\u{2028}'|'\u{2029}')
        {
            out.push_str(&format!("\\u{:04x}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn error(code: &str) -> String {
    Connection::open_in_memory().and_then(|conn|conn.query_row(
        "SELECT json_object('protocol','swapctx.context','version',1,'ok',json('false'),'error',json_object('code',?1,'message','Context unavailable; no context was delivered'),'records',json('[]'))",
        [code],|r|r.get::<_,String>(0)))
        .unwrap_or_else(|_|r#"{"protocol":"swapctx.context","version":1,"ok":false,"error":{"code":"serialization_failed","message":"Context unavailable"},"records":[]}"#.into())
}

pub fn execute(path: Result<std::path::PathBuf>, request: Request) -> u8 {
    if request.protocol_version != 1 {
        println!("{}", error("unsupported_protocol"));
        return 1;
    }
    if request.query.as_ref().is_some_and(|q| {
        crate::validate_text("Query", q, 4096).is_err()
            || crate::sensitive::guard(q, false).is_err()
            || q.split_whitespace().count() > 16
    }) {
        println!("{}", error("invalid_request"));
        return 1;
    }
    let Ok(path) = path else {
        println!("{}", error("invalid_storage_path"));
        return 1;
    };
    let Ok(mut conn) = crate::storage::open(&path, false, false) else {
        println!("{}", error("store_unavailable"));
        return 1;
    };
    if crate::storage::version(&conn).ok() != Some(3) {
        println!("{}", error("migration_required"));
        return 1;
    }
    let Ok(cwd) = std::env::current_dir().and_then(|p| p.canonicalize()) else {
        println!("{}", error("project_unavailable"));
        return 1;
    };
    match packet(&mut conn, &cwd, &request) {
        Ok(output) => {
            println!("{output}");
            0
        }
        Err(_) => {
            println!("{}", error("retrieval_failed"));
            1
        }
    }
}

#[derive(Clone, Default)]
struct Stats {
    requirements: i64,
    rules: i64,
    stale: i64,
    conflicts: i64,
    unknown: i64,
    sensitive: i64,
    budget: i64,
    limit: i64,
    scanned: i64,
    scan_limited: bool,
    inactive: i64,
}
struct Item {
    json: String,
    requirement: bool,
    review: bool,
}
// Rank lightweight identities, not thousands of potentially large JSON records.
struct Candidate {
    score: usize,
    id: i64,
    requirement: bool,
    stale: bool,
    conflict: bool,
    unknown: bool,
}

fn remaining_stats(stats: &Stats, remaining: usize) -> Stats {
    let mut reserved = stats.clone();
    // Reserve the maximum future counter widths before deciding if a record fits.
    // Only the actual counters are returned. This can leave a few bytes unused.
    reserved.budget += remaining as i64;
    reserved.limit += remaining as i64;
    reserved
}

fn envelope(
    conn: &Connection,
    root: &str,
    request: &Request,
    timestamp: i64,
    stats: &Stats,
    selection: (&[Item], &[String]),
    used: usize,
) -> Result<String> {
    let (records, rules) = selection;
    let required = records.iter().filter(|r| r.requirement).count();
    let review = records.iter().filter(|r| r.review).count();
    let complete = required as i64 == stats.requirements
        && rules.len() as i64 == stats.rules
        && !stats.scan_limited
        && review == 0;
    let record_json = format!(
        "[{}]",
        records
            .iter()
            .map(|r| r.json.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let rule_json = format!("[{}]", rules.join(","));
    let output=conn.query_row("SELECT json_object(
        'protocol','swapctx.context','version',1,'ok',json('true'),'generated_unix',?1,'schema_version',3,
        'project',json_object('root',?2),
        'policy',json_object('ranking','requirements_then_term_matches_then_recency','fresh_only',json(?3),'conflicts_excluded',json(?4),'unknown_provenance_excluded',json(?5),'secret_like_records_excluded',json('true'),'scan_limit',5000),
        'budget',json_object('max_bytes',?6,'bytes_used',?7,'max_records',?8,'unit','utf8_json_bytes'),
        'coverage',json_object('active_valid_requirements',?9,'requirements_returned',?10,'boundary_rules_total',?11,'boundary_rules_returned',?12,'mandatory_context_complete',json(?13),'review_required_records',?14),
        'omitted',json_object('inactive_or_invalid',?15,'stale',?16,'conflicted',?17,'unknown_provenance',?18,'sensitive',?19,'budget',?20,'record_limit',?21),
        'scan',json_object('candidates_examined',?22,'truncated',json(?23)),
        'records',json(?24),'boundary_rules',json(?25),
        'limitations',json_object('provenance_authenticated',json('false'),'native_context_expansion',json('false'),'enforcement',json('false'),'agent_receipt_verified',json('false'),'semantic_ranking',json('false')))",
        params![timestamp,root,(!request.include_stale).to_string(),(!request.include_conflicts).to_string(),(!request.allow_unknown_provenance).to_string(),request.max_bytes,used as i64,request.limit,stats.requirements,required as i64,stats.rules,rules.len() as i64,complete.to_string(),review as i64,stats.inactive,stats.stale,stats.conflicts,stats.unknown,stats.sensitive,stats.budget,stats.limit,stats.scanned,stats.scan_limited.to_string(),record_json,rule_json],|r|r.get::<_,String>(0))?;
    Ok(safe_json(output))
}

fn sized(
    conn: &Connection,
    root: &str,
    request: &Request,
    timestamp: i64,
    stats: &Stats,
    records: &[Item],
    rules: &[String],
) -> Result<String> {
    let mut used = 0;
    for _ in 0..8 {
        let output = envelope(
            conn,
            root,
            request,
            timestamp,
            stats,
            (records, rules),
            used,
        )?;
        if output.len() == used {
            return Ok(output);
        }
        used = output.len();
    }
    bail!("JSON size did not converge")
}

pub fn packet(conn: &mut Connection, cwd: &Path, request: &Request) -> Result<String> {
    let tx = conn.transaction()?;
    let output = packet_in_snapshot(&tx, cwd, request, crate::memory::now()?)?;
    tx.commit()?;
    Ok(output)
}

/// Caller owns a read transaction so counts and coverage use the same snapshot.
pub(crate) fn packet_in_snapshot(
    tx: &Connection,
    cwd: &Path,
    request: &Request,
    timestamp: i64,
) -> Result<String> {
    if crate::storage::version(tx)? != 3 {
        bail!("Context requires schema 3; back up and run an explicit write command to migrate");
    }
    if request.protocol_version != 1
        || !(2048..=131072).contains(&request.max_bytes)
        || !(1..=100).contains(&request.limit)
    {
        bail!("Invalid context request bounds or protocol");
    }
    if let Some(query) = &request.query {
        crate::validate_text("Query", query, 4096)?;
        crate::sensitive::guard(query, false)?;
        if query.split_whitespace().count() > 16 {
            bail!("Too many query terms");
        }
    }
    crate::storage::diagnose(tx)?;
    let (project, root) = crate::storage::project(tx, cwd)?;
    let root = crate::paths::path_text(&root)?;
    let mut terms = request
        .query
        .as_ref()
        .map(|q| {
            q.split_whitespace()
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    terms.sort();
    terms.dedup();
    let mut stats = Stats {
    requirements: tx.query_row("SELECT count(*) FROM memories m LEFT JOIN memory_metadata d ON d.memory_id=m.id WHERE m.project_id=?1 AND m.kind='requirement' COLLATE NOCASE AND coalesce(d.state,'active')='active' AND coalesce(d.validity,'valid')='valid'",[project],|r|r.get(0))?,
    inactive: tx.query_row("SELECT count(*) FROM memories m LEFT JOIN memory_metadata d ON d.memory_id=m.id WHERE m.project_id=?1 AND (coalesce(d.state,'active')!='active' OR coalesce(d.validity,'valid')!='valid')",[project],|r|r.get(0))?,
    rules: tx.query_row(
        "SELECT count(*) FROM rules WHERE project_id=?1",
        [project],
        |r| r.get(0),
    )?,
    ..Stats::default()
    };
    let mut rules = Vec::new();
    let mut statement=tx.prepare("SELECT r.id,r.pattern,r.reason,coalesce(p.policy,'deny') FROM rules r LEFT JOIN rule_policies p ON p.rule_id=r.id WHERE r.project_id=?1 ORDER BY r.id LIMIT 5001")?;
    let mut cursor = statement.query([project])?;
    while let Some(row) = cursor.next()? {
        let id: i64 = row.get(0)?;
        let pattern: String = row.get(1)?;
        let reason: String = row.get(2)?;
        let policy: String = row.get(3)?;
        crate::paths::rule_matcher(&pattern)?;
        if [&pattern, &reason]
            .iter()
            .any(|s| crate::sensitive::guard(s, false).is_err())
        {
            stats.sensitive += 1;
            continue;
        }
        let json:String=tx.query_row("SELECT json_object('id',CAST(?1 AS TEXT),'pattern',?2,'reason',?3,'policy',?4,'enforced',json('false'))",params![id,pattern,reason,policy],|r|r.get(0))?;
        // Never truncate rule semantics. Bound staging independently of stored size.
        if json.len() > request.max_bytes as usize {
            stats.budget += 1;
            continue;
        }
        rules.push(json);
        if rules.len() > 100 {
            break;
        } // Packet explicitly reports incomplete rule coverage.
    }
    drop(cursor);
    drop(statement);
    let competing = {
        let mut stmt=tx.prepare("SELECT d.memory_key,count(*) FROM memory_metadata d JOIN memories m ON m.id=d.memory_id WHERE m.project_id=?1 AND d.memory_key IS NOT NULL AND d.state='active' AND d.validity='valid' GROUP BY d.memory_key HAVING count(*)>1")?;
        let result = stmt
            .query_map([project], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<std::collections::HashMap<_, _>>>()?;
        result
    };
    let mut values = vec![Value::Integer(project)];
    let mut matches = Vec::new();
    for term in &terms {
        values.push(Value::Text(format!("%{}%", crate::escape_like(term))));
        let n = values.len();
        matches.push(format!("m.content LIKE ?{n} ESCAPE '\\' OR m.kind LIKE ?{n} ESCAPE '\\' OR d.memory_key LIKE ?{n} ESCAPE '\\'"));
    }
    let relevance = if matches.is_empty() {
        "1".into()
    } else {
        format!("({})", matches.join(" OR "))
    };
    let sql=format!("SELECT m.id,m.kind,m.content,m.created_at,d.memory_key,d.expires_at,o.source,o.actor FROM memories m LEFT JOIN memory_metadata d ON d.memory_id=m.id JOIN memory_origins o ON o.memory_id=m.id WHERE m.project_id=?1 AND coalesce(d.state,'active')='active' AND coalesce(d.validity,'valid')='valid' AND (m.kind='requirement' COLLATE NOCASE OR {relevance}) ORDER BY (m.kind='requirement' COLLATE NOCASE) DESC,m.id DESC LIMIT 5001");
    let mut stmt = tx.prepare(&sql)?;
    let mut cursor = stmt.query(params_from_iter(values))?;
    let mut candidates = Vec::new();
    while let Some(row) = cursor.next()? {
        if stats.scanned == 5000 {
            stats.scan_limited = true;
            break;
        }
        stats.scanned += 1;
        let id: i64 = row.get(0)?;
        let kind: String = row.get(1)?;
        let content: String = row.get(2)?;
        let created: String = row.get(3)?;
        let key: Option<String> = row.get(4)?;
        let expires: Option<i64> = row.get(5)?;
        let source: String = row.get(6)?;
        let actor: String = row.get(7)?;
        let stale = expires.is_some_and(|e| e <= timestamp);
        let conflict = key.as_ref().is_some_and(|k| competing.contains_key(k));
        let unknown = source == "legacy-unknown" || actor == "unknown";
        // A single primary omission reason; mandatory counts independently reveal every gap.
        if [
            &kind,
            &content,
            &created,
            &source,
            &actor,
            key.as_deref().unwrap_or(""),
        ]
        .iter()
        .any(|s| crate::sensitive::guard(s, false).is_err())
        {
            stats.sensitive += 1;
            continue;
        }
        if stale && !request.include_stale {
            stats.stale += 1;
            continue;
        }
        if conflict && !request.include_conflicts {
            stats.conflicts += 1;
            continue;
        }
        if unknown && !request.allow_unknown_provenance {
            stats.unknown += 1;
            continue;
        }
        if content.len() + source.len() + actor.len() > request.max_bytes as usize {
            stats.budget += 1;
            continue;
        }
        let searchable = format!(
            "{} {} {}",
            content.to_ascii_lowercase(),
            kind.to_ascii_lowercase(),
            key.as_deref().unwrap_or("").to_ascii_lowercase()
        );
        let score = terms
            .iter()
            .filter(|t| searchable.contains(t.as_str()))
            .count();
        let requirement = kind.eq_ignore_ascii_case("requirement");
        candidates.push(Candidate {
            requirement,
            score,
            id,
            stale,
            conflict,
            unknown,
        });
    }
    drop(cursor);
    drop(stmt);
    candidates.sort_unstable_by_key(|r| std::cmp::Reverse((r.requirement, r.score, r.id)));
    let mut selected = Vec::new();
    // Rules retain priority. Reserve future omission-counter widths before packing.
    while sized(
        tx,
        root,
        request,
        timestamp,
        &remaining_stats(&stats, candidates.len()),
        &selected,
        &rules,
    )?
    .len()
        >= request.max_bytes as usize
    {
        if rules.pop().is_none() {
            bail!("Budget too small for metadata");
        }
        stats.budget += 1;
    }
    // An empty envelope is a lower bound for every subsequent packet (except the
    // one-byte false -> true coverage change). Quickly skip obvious non-fits.
    let minimum = envelope(tx, root, request, timestamp, &stats, (&[], &rules), 0)?
        .len()
        .saturating_sub(1);
    let mut payload_bytes = 0;
    let mut encode = tx.prepare("SELECT json_object('id',CAST(m.id AS TEXT),'kind',m.kind,'content',m.content,'created_at',m.created_at,'key',d.memory_key,'expires_unix',d.expires_at,'reviewed_unix',d.reviewed_at,'origin',json_object('id',o.origin_id,'source',o.source,'actor',o.actor,'registered_unix',o.recorded_at,'authenticated',json('false')),'freshness',?3,'conflicted',json(?4),'provenance_status',?5,'matched_terms',?6,'selection_reason',?7) FROM memories m LEFT JOIN memory_metadata d ON d.memory_id=m.id JOIN memory_origins o ON o.memory_id=m.id WHERE m.id=?1 AND m.project_id=?2")?;
    for (index, candidate) in candidates.iter().enumerate() {
        let remaining = candidates.len() - index;
        if selected.len() == request.limit as usize {
            stats.limit += remaining as i64;
            break;
        }
        // Re-read only candidate values needed for packing, within the SAME snapshot.
        let json = safe_json(encode.query_row(
            params![
                candidate.id,
                project,
                if candidate.stale { "stale" } else { "fresh" },
                candidate.conflict.to_string(),
                if candidate.unknown {
                    "unknown"
                } else {
                    "self_reported"
                },
                candidate.score as i64,
                if candidate.requirement {
                    "project_requirement"
                } else if terms.is_empty() {
                    "recent"
                } else {
                    "query_terms"
                }
            ],
            |r| r.get(0),
        )?);
        let added_bytes = json.len() + usize::from(!selected.is_empty());
        if minimum + payload_bytes + added_bytes >= request.max_bytes as usize {
            stats.budget += 1;
            continue;
        }
        selected.push(Item {
            json,
            requirement: candidate.requirement,
            review: candidate.stale || candidate.conflict || candidate.unknown,
        });
        let reserved_packet = sized(
            tx,
            root,
            request,
            timestamp,
            &remaining_stats(&stats, remaining),
            &selected,
            &rules,
        )?;
        if reserved_packet.len() < request.max_bytes as usize {
            payload_bytes += added_bytes;
        } else {
            selected.pop();
            stats.budget += 1;
        }
    }
    drop(encode);
    let output = sized(tx, root, request, timestamp, &stats, &selected, &rules)?;
    if output.len() >= request.max_bytes as usize {
        bail!("Budget accounting failed");
    }
    Ok(output)
}
