//! Versioned SQLite bundles. Untrusted input is read as values, never executed as SQL.
use crate::{history, sensitive, storage, validate_text};
use anyhow::{bail, Context, Result};
use rusqlite::{params, params_from_iter, types::Value, Connection, TransactionBehavior};
use std::{
    collections::{HashMap, HashSet},
    fs,
    fs::OpenOptions,
    path::Path,
};

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;

fn rows(conn: &Connection, sql: &str, project: i64) -> Result<Vec<Vec<Value>>> {
    let mut stmt = conn.prepare(sql)?;
    let count = stmt.column_count();
    let mut cursor = stmt.query([project])?;
    let mut result = Vec::new();
    let mut bytes = 0usize;
    while let Some(row) = cursor.next()? {
        if result.len() >= MAX_ROWS {
            bail!("Transfer row limit exceeded (100000 per table)");
        }
        let values = (0..count)
            .map(|i| row.get::<_, Value>(i))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        bytes += values
            .iter()
            .map(|v| match v {
                Value::Text(s) => s.len(),
                Value::Blob(b) => b.len(),
                _ => 8,
            })
            .sum::<usize>();
        if bytes > 64 * 1024 * 1024 {
            bail!("Transfer decoded table limit exceeded (64 MiB)");
        }
        result.push(values);
    }
    Ok(result)
}
fn integer(value: &Value) -> Result<i64> {
    if let Value::Integer(v) = value {
        Ok(*v)
    } else {
        bail!("Invalid integer in transfer")
    }
}
fn text(value: &Value) -> Result<&str> {
    if let Value::Text(v) = value {
        Ok(v)
    } else {
        bail!("Invalid text in transfer")
    }
}
fn put(conn: &Connection, table: &str, columns: &str, values: &[Value]) -> Result<()> {
    let placeholders = (0..values.len()).map(|_| "?").collect::<Vec<_>>().join(",");
    conn.execute(
        &format!("INSERT INTO {table}({columns}) VALUES({placeholders})"),
        params_from_iter(values),
    )?;
    Ok(())
}

pub fn export(source: &mut Connection, project: i64, destination: &Path) -> Result<()> {
    crate::paths::path_text(destination)?;
    if destination.try_exists()? {
        bail!("Export destination exists; nothing was overwritten");
    }
    let read = source.transaction()?;
    if storage::version(&read)? < 3 {
        bail!("Export requires stable origins; run init to migrate this legacy store first");
    }
    storage::diagnose(&read)?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staged = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
    let mut target = storage::open(&staged, true, true)?;
    let tx = target.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for row in rows(
        &read,
        "SELECT id,path,created_at FROM projects WHERE id=?1",
        project,
    )? {
        put(&tx, "projects", "id,path,created_at", &row)?;
    }
    for (table, cols, sql) in [
        (
            "memories",
            "id,project_id,kind,content,created_at",
            "SELECT * FROM memories WHERE project_id=?1 ORDER BY id",
        ),
        (
            "rules",
            "id,project_id,pattern,reason,created_at",
            "SELECT * FROM rules WHERE project_id=?1 ORDER BY id",
        ),
    ] {
        for row in rows(&read, sql, project)? {
            put(&tx, table, cols, &row)?;
        }
    }
    if storage::version(&read)? >= 2 {
        for row in rows(
            &read,
            "SELECT p.* FROM rule_policies p JOIN rules r ON r.id=p.rule_id WHERE r.project_id=?1",
            project,
        )? {
            put(&tx, "rule_policies", "rule_id,policy", &row)?;
        }
        let cols = if storage::version(&read)? >= 3 {
            "memory_id,state,memory_key,expires_at,reviewed_at,superseded_by,validity,invalid_reason"
        } else {
            "memory_id,state,memory_key,expires_at,reviewed_at,superseded_by"
        };
        for row in rows(&read,"SELECT d.* FROM memory_metadata d JOIN memories m ON m.id=d.memory_id WHERE m.project_id=?1",project)? {put(&tx,"memory_metadata",cols,&row)?;}
    }
    if storage::version(&read)? >= 3 {
        for (table,cols,sql) in [
            ("memory_origins","memory_id,origin_id,source,actor,recorded_at","SELECT o.* FROM memory_origins o JOIN memories m ON m.id=o.memory_id WHERE m.project_id=?1"),
            ("memory_events","id,memory_id,action,detail,recorded_at","SELECT e.* FROM memory_events e JOIN memories m ON m.id=e.memory_id WHERE m.project_id=?1 ORDER BY e.id"),
        ] {for row in rows(&read,sql,project)? {put(&tx,table,cols,&row)?;} }
    } else {
        for row in rows(
            &read,
            "SELECT id FROM memories WHERE project_id=?1",
            project,
        )? {
            let id = integer(&row[0])?;
            history::origin(&tx, id, "legacy-unknown", "unknown")?;
            history::event(
                &tx,
                id,
                "legacy",
                "Prior origin and action history unavailable",
            )?;
        }
    }
    storage::diagnose(&tx)?;
    tx.commit()?;
    drop(target);
    read.commit()?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&staged)?
        .sync_all()?;
    staged
        .persist_noclobber(destination)
        .map_err(|e| e.error)
        .context("Cannot publish export; no file overwritten")?;
    println!("Exported selected project's plaintext context; includes retained history and boundary rules");
    Ok(())
}

fn signature(conn: &Connection, id: i64) -> Result<Vec<Vec<Value>>> {
    let mut result=rows(conn,"SELECT m.kind,m.content,m.created_at,o.origin_id,o.source,o.actor,o.recorded_at,
        coalesce(d.state,'active'),d.memory_key,d.expires_at,d.reviewed_at,replacement.origin_id,coalesce(d.validity,'valid'),d.invalid_reason
        FROM memories m JOIN memory_origins o ON o.memory_id=m.id LEFT JOIN memory_metadata d ON d.memory_id=m.id LEFT JOIN memory_origins replacement ON replacement.memory_id=d.superseded_by WHERE m.id=?1",id)?;
    result.extend(rows(conn,"SELECT action,detail,recorded_at FROM memory_events WHERE memory_id=?1 AND action!='import' ORDER BY id",id)?);
    Ok(result)
}

pub fn import(
    target: &mut Connection,
    project: i64,
    source_path: &Path,
    apply: bool,
    allow: bool,
) -> Result<()> {
    if fs::metadata(source_path)?.len() > MAX_BYTES {
        bail!("Import file exceeds 256 MiB limit");
    }
    let mut source = storage::open(source_path, false, false)?;
    let source = source.transaction()?;
    storage::diagnose(&source)?;
    if storage::version(&source)? != 3 {
        bail!("Import requires a schema-3 project bundle; export it with this version first");
    }
    let projects = rows(&source, "SELECT id FROM projects WHERE ?1 IS NOT NULL", 0)?;
    if projects.len() != 1 {
        bail!(
            "Import requires exactly one project; use project export, not a multi-project backup"
        );
    }
    let source_project = integer(&projects[0][0])?;
    let memories = rows(
        &source,
        "SELECT * FROM memories WHERE project_id=?1 ORDER BY id",
        source_project,
    )?;
    let origins=rows(&source,"SELECT o.* FROM memory_origins o JOIN memories m ON m.id=o.memory_id WHERE m.project_id=?1",source_project)?;
    let metadata=rows(&source,"SELECT d.* FROM memory_metadata d JOIN memories m ON m.id=d.memory_id WHERE m.project_id=?1",source_project)?;
    let events=rows(&source,"SELECT e.* FROM memory_events e JOIN memories m ON m.id=e.memory_id WHERE m.project_id=?1 ORDER BY e.id",source_project)?;
    let rules=rows(&source,"SELECT r.id,r.pattern,r.reason,r.created_at,coalesce(p.policy,'deny') FROM rules r LEFT JOIN rule_policies p ON p.rule_id=r.id WHERE r.project_id=?1 ORDER BY r.id",source_project)?;
    // Bounds and secret guards apply even to preview; no untrusted SQL is executed.
    for row in &memories {
        for (i, max) in [(2, 128), (3, 1024 * 1024), (4, 4096)] {
            let v = text(&row[i])?;
            validate_text("Imported memory", v, max)?;
            sensitive::guard(v, allow)?;
        }
    }
    for row in &origins {
        for i in [2, 3] {
            let v = text(&row[i])?;
            validate_text("Imported provenance", v, 4096)?;
            sensitive::guard(v, allow)?;
        }
    }
    for row in &metadata {
        for (i, max) in [(2, 128), (7, 16384)] {
            if row[i] != Value::Null {
                let v = text(&row[i])?;
                validate_text("Imported metadata", v, max)?;
                sensitive::guard(v, allow)?;
            }
        }
    }
    for row in &events {
        let v = text(&row[3])?;
        validate_text("Imported history", v, 16384)?;
        sensitive::guard(v, allow)?;
    }
    for row in &rules {
        crate::paths::rule_matcher(text(&row[1])?)?;
        validate_text("Imported rule reason", text(&row[2])?, 16384)?;
        sensitive::guard(text(&row[1])?, allow)?;
        sensitive::guard(text(&row[2])?, allow)?;
    }
    let tx = target.transaction_with_behavior(if apply {
        TransactionBehavior::Immediate
    } else {
        TransactionBehavior::Deferred
    })?;
    let mut mapped = HashMap::new();
    let mut new = HashSet::new();
    for row in &origins {
        let old = integer(&row[0])?;
        let existing = if storage::version(&tx)? >= 3 {
            // Origin lookup uses a text parameter; IDs never cross projects implicitly.
            use rusqlite::OptionalExtension;
            tx.query_row("SELECT m.id,m.project_id FROM memory_origins o JOIN memories m ON m.id=o.memory_id WHERE o.origin_id=?1",[text(&row[1])?],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).optional()?
        } else {
            None
        };
        if let Some((id, owner)) = existing {
            if owner != project {
                bail!("Origin already belongs to another project; import refused");
            }
            if signature(&source, old)? != signature(&tx, id)? {
                bail!("Divergent history or metadata for an existing origin; import refused without changing records");
            }
            mapped.insert(old, id);
        } else {
            new.insert(old);
        }
    }
    println!(
        "Import plan: {} new memories, {} identical origins skipped, {} candidate rules",
        new.len(),
        mapped.len(),
        rules.len()
    );
    if !apply {
        println!("Preview only; use --apply to commit. Input and stored context remain unchanged.");
        return Ok(());
    }
    for row in &memories {
        let old = integer(&row[0])?;
        if !new.contains(&old) {
            continue;
        }
        tx.execute(
            "INSERT INTO memories(project_id,kind,content,created_at) VALUES(?1,?2,?3,?4)",
            params![project, row[2], row[3], row[4]],
        )?;
        mapped.insert(old, tx.last_insert_rowid());
    }
    for row in &origins {
        let old = integer(&row[0])?;
        if new.contains(&old) {
            let mut copy = row.clone();
            copy[0] = Value::Integer(mapped[&old]);
            put(
                &tx,
                "memory_origins",
                "memory_id,origin_id,source,actor,recorded_at",
                &copy,
            )?;
        }
    }
    for row in &metadata {
        let old = integer(&row[0])?;
        if !new.contains(&old) {
            continue;
        }
        let mut copy = row.clone();
        copy[0] = Value::Integer(mapped[&old]);
        if copy[5] != Value::Null {
            copy[5] = Value::Integer(
                *mapped
                    .get(&integer(&copy[5])?)
                    .context("Missing imported replacement")?,
            );
        }
        put(&tx,"memory_metadata","memory_id,state,memory_key,expires_at,reviewed_at,superseded_by,validity,invalid_reason",&copy)?;
    }
    for row in &events {
        let old = integer(&row[1])?;
        if !new.contains(&old) {
            continue;
        }
        let mut copy = row[1..].to_vec();
        copy[0] = Value::Integer(mapped[&old]);
        put(
            &tx,
            "memory_events",
            "memory_id,action,detail,recorded_at",
            &copy,
        )?;
    }
    for old in &new {
        history::event(
            &tx,
            mapped[old],
            "import",
            "Imported with original self-reported provenance; local IDs remapped",
        )?;
    }
    for row in rules {
        let exists: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM rules r LEFT JOIN rule_policies p ON p.rule_id=r.id WHERE r.project_id=?1 AND r.pattern=?2 AND r.reason=?3 AND coalesce(p.policy,'deny')=?4)",params![project,row[1],row[2],row[4]],|r|r.get(0))?;
        if exists {
            continue;
        }
        tx.execute(
            "INSERT INTO rules(project_id,pattern,reason,created_at) VALUES(?1,?2,?3,?4)",
            params![project, row[1], row[2], row[3]],
        )?;
        tx.execute(
            "INSERT INTO rule_policies(rule_id,policy) VALUES(?1,?2)",
            params![tx.last_insert_rowid(), row[4]],
        )?;
    }
    storage::diagnose(&tx)?;
    tx.commit()?;
    println!("Import committed atomically; existing history retained");
    Ok(())
}

pub fn restore(source_path: &Path, destination: &Path) -> Result<()> {
    let source = storage::open(source_path, false, false)?;
    storage::diagnose(&source)?;
    let directory = destination.parent().context("Invalid restore directory")?;
    fs::create_dir(directory).context("Restore requires a new directory with an existing parent; existing directories are never replaced")?;
    // Interruption may leave this new directory or staging files, never replace user data.
    crate::backup::snapshot(&source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_full_during_real_import_rolls_back_all_context_tables() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("source.db");
        let mut source = storage::open(&source_path, true, true).unwrap();
        source
            .execute("INSERT INTO projects(path) VALUES('source')", [])
            .unwrap();
        let tx = source.transaction().unwrap();
        tx.execute(
            "INSERT INTO memories(project_id,kind,content) VALUES(1,'requirement',?1)",
            ["context ".repeat(65536)],
        )
        .unwrap();
        history::origin(&tx, 1, "specification", "test").unwrap();
        history::event(&tx, 1, "add", "Fixture").unwrap();
        tx.commit().unwrap();
        drop(source);
        let mut target = storage::open(&temp.path().join("target.db"), true, true).unwrap();
        target
            .execute("INSERT INTO projects(path) VALUES('target')", [])
            .unwrap();
        let pages: i64 = target
            .pragma_query_value(None, "page_count", |r| r.get(0))
            .unwrap();
        target.pragma_update(None, "max_page_count", pages).unwrap();
        let error = import(&mut target, 1, &source_path, true, false).unwrap_err();
        let sqlite = error.downcast_ref::<rusqlite::Error>().unwrap();
        assert!(
            matches!(sqlite,rusqlite::Error::SqliteFailure(code,_) if code.code==rusqlite::ErrorCode::DiskFull)
        );
        for table in [
            "memories",
            "memory_metadata",
            "memory_origins",
            "memory_events",
            "rules",
        ] {
            let count: i64 = target
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0);
        }
        storage::diagnose(&target).unwrap();
        target.pragma_update(None, "max_page_count", 10000).unwrap();
        import(&mut target, 1, &source_path, true, false).unwrap();
        storage::diagnose(&target).unwrap();
    }
}
