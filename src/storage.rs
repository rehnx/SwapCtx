use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use crate::paths::path_text;

const APPLICATION_ID: i64 = 0x53574358;
const SCHEMA_VERSION: i64 = 1;

pub fn open(path: &Path, create: bool, writable: bool) -> Result<Connection> {
    if create {
        fs::create_dir_all(path.parent().context("Invalid database path")?)?;
    }
    let flags = if create {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else if writable {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let mut conn = Connection::open_with_flags(path, flags)
        .context("Cannot open local database; run 'swapctx init' for first use")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    if create {
        initialize(&mut conn)?;
    } else {
        validate(&conn, false)?;
    }
    Ok(conn)
}

fn validate(conn: &Connection, allow_empty: bool) -> Result<()> {
    let app: i64 = conn.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if app != 0 && app != APPLICATION_ID {
        bail!("Database belongs to a different application; refusing to modify it");
    }
    if !(0..=SCHEMA_VERSION).contains(&version) {
        bail!("Unsupported database schema version {version}; upgrade SwapCtx");
    }
    let tables: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [], |row| row.get(0),
    )?;
    if tables == 0 && version == 0 && app == 0 && allow_empty {
        return Ok(());
    }
    if tables != 3 || (version == SCHEMA_VERSION && app != APPLICATION_ID) {
        bail!("Unrecognized or incomplete SwapCtx database; preserve it and restore a valid backup");
    }
    let executable_schema: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type IN ('trigger', 'view')",
        [], |r| r.get(0),
    )?;
    if executable_schema != 0 {
        bail!("Unexpected database triggers or views; refusing this schema");
    }
    // Recognize the original unversioned schema without rewriting saved records.
    for sql in [
        "SELECT id, path, created_at FROM projects LIMIT 0",
        "SELECT id, project_id, pattern, reason, created_at FROM rules LIMIT 0",
        "SELECT id, project_id, kind, content, created_at FROM memories LIMIT 0",
    ] {
        conn.prepare(sql).context("Incomplete SwapCtx schema; no records were modified")?;
    }
    for (table, columns) in [
        ("projects", &["id", "path", "created_at"][..]),
        ("rules", &["id", "project_id", "pattern", "reason", "created_at"][..]),
        ("memories", &["id", "project_id", "kind", "content", "created_at"][..]),
    ] {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, bool>(3)?, r.get::<_, i64>(5)?)))?;
        let actual = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        let invalid = actual.iter().zip(columns).enumerate().any(|(i, ((name, kind, required, pk), expected))| {
            if name.as_str() != *expected { return true; }
            if i == 0 { return kind != "INTEGER" || *pk != 1; }
            let expected_type = if name == "project_id" { "INTEGER" } else { "TEXT" };
            kind != expected_type || !*required || *pk != 0
        });
        if actual.len() != columns.len() || invalid {
            bail!("Unrecognized columns in {table}; refusing to modify this database");
        }
    }
    for table in ["rules", "memories"] {
        let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list({table})"))?;
        let keys = stmt.query_map([], |r| Ok((r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(6)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let expected = ("projects".to_owned(), "project_id".to_owned(), "id".to_owned(), "CASCADE".to_owned());
        if keys.len() != 1 || keys[0] != expected {
            bail!("Missing project isolation constraint in {table}");
        }
    }
    let unique_path: bool = conn.query_row(
        "SELECT EXISTS(
             SELECT i.name FROM pragma_index_list('projects') AS i
             JOIN pragma_index_info(i.name) AS c
             WHERE i.[unique] = 1 AND i.partial = 0
             GROUP BY i.name HAVING count(*) = 1 AND max(c.name = 'path') = 1
         )",
        [], |r| r.get(0),
    )?;
    if !unique_path { bail!("Missing unique project identity constraint"); }
    #[cfg(windows)]
    {
        let duplicate_identity: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects GROUP BY path COLLATE NOCASE HAVING count(*) > 1)",
            [], |r| r.get(0),
        )?;
        if duplicate_identity {
            bail!("Ambiguous Windows project identities; preserve this database and resolve duplicate registrations");
        }
    }
    Ok(())
}

fn initialize(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    validate(&tx, true)?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
             id INTEGER PRIMARY KEY,
             path TEXT NOT NULL UNIQUE,
             created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS rules (
             id INTEGER PRIMARY KEY,
             project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
             pattern TEXT NOT NULL,
             reason TEXT NOT NULL,
             created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS memories (
             id INTEGER PRIMARY KEY,
             project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
             kind TEXT NOT NULL,
             content TEXT NOT NULL,
             created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE INDEX IF NOT EXISTS projects_path_nocase_idx ON projects(path COLLATE NOCASE);
         CREATE INDEX IF NOT EXISTS rules_project_idx ON rules(project_id);
         CREATE INDEX IF NOT EXISTS memories_project_idx ON memories(project_id);",
    )?;
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

pub fn register(conn: &mut Connection, root: &Path) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let query = if cfg!(windows) {
        "SELECT EXISTS(SELECT 1 FROM projects WHERE path = ?1 COLLATE NOCASE)"
    } else {
        "SELECT EXISTS(SELECT 1 FROM projects WHERE path = ?1)"
    };
    let exists: bool = tx.query_row(query, params![path_text(root)?], |r| r.get(0))?;
    if !exists {
        tx.execute(
        "INSERT INTO projects(path) VALUES (?1) ON CONFLICT(path) DO NOTHING",
        params![path_text(root)?],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Nearest registered ancestor wins, so nested projects remain isolated.
pub fn project(conn: &Connection, cwd: &Path) -> Result<(i64, PathBuf)> {
    for root in cwd.ancestors() {
        let query = if cfg!(windows) {
            "SELECT id FROM projects WHERE path = ?1 COLLATE NOCASE"
        } else {
            "SELECT id FROM projects WHERE path = ?1"
        };
        let id = conn.query_row(
            query,
            params![path_text(root)?], |row| row.get(0),
        ).optional()?;
        if let Some(id) = id {
            return Ok((id, root.to_path_buf()));
        }
    }
    bail!("No registered project contains the current folder. Run 'swapctx init' at the project root.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialization_is_versioned_and_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&mut conn).unwrap();
        conn.execute("INSERT INTO projects(path) VALUES ('example')", []).unwrap();
        conn.execute("INSERT INTO memories(project_id,kind,content) VALUES (1,'requirement','keep')", []).unwrap();
        conn.pragma_update(None, "user_version", 0).unwrap();
        conn.pragma_update(None, "application_id", 0).unwrap();
        initialize(&mut conn).unwrap();
        initialize(&mut conn).unwrap();
        assert_eq!(conn.query_row("SELECT content FROM memories", [], |r| r.get::<_, String>(0)).unwrap(), "keep");
        assert_eq!(conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 1);
    }

    #[test]
    fn rejects_foreign_and_future_schemas() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE other(data TEXT)", []).unwrap();
        assert!(initialize(&mut conn).is_err());
        assert_eq!(conn.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table'", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&mut conn).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(initialize(&mut conn).is_err());
    }

    #[test]
    fn rejects_unexpected_triggers_before_writing() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&mut conn).unwrap();
        conn.execute_batch("CREATE TRIGGER unexpected AFTER INSERT ON memories BEGIN DELETE FROM memories; END;").unwrap();
        assert!(initialize(&mut conn).is_err());
        assert!(validate(&conn, false).is_err());
    }

    #[test]
    fn migration_rollback_preserves_original_database() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../tests/fixtures/v0.sql")).unwrap();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
        tx.pragma_update(None, "user_version", SCHEMA_VERSION).unwrap();
        assert!(tx.execute("INSERT INTO missing VALUES (1)", []).is_err());
        drop(tx);
        assert_eq!(conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 0);
        validate(&conn, false).unwrap();
    }

    #[test]
    fn missing_database_is_never_created_by_reads_or_writes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing.db");
        assert!(open(&path, false, false).is_err());
        assert!(open(&path, false, true).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn nearest_registered_root_wins() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&mut conn).unwrap();
        register(&mut conn, &root).unwrap();
        register(&mut conn, &root.join("nested")).unwrap();
        assert_eq!(project(&conn, &root.join("nested/src")).unwrap().1, root.join("nested"));
        assert_eq!(project(&conn, &root.join("other")).unwrap().1, root);
    }
}
