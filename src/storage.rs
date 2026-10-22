use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use crate::paths::path_text;

const APPLICATION_ID: i64 = 0x53574358;
pub const SCHEMA_VERSION: i64 = 3;

pub fn version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

pub fn open(path: &Path, create: bool, writable: bool) -> Result<Connection> {
    path_text(path)?;
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
    // Windows writer bursts can outlast five seconds under filesystem/host load.
    // Queue writes for a bounded interval without replaying an operation that
    // may have committed. Keep short read waits for agent callback deadlines.
    conn.busy_timeout(Duration::from_secs(if create || writable { 30 } else { 5 }))?;
    secure_connection(&conn)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    if create {
        initialize(&mut conn)?;
    } else {
        validate(&conn, false)?;
        if writable && version(&conn)? < SCHEMA_VERSION {
            initialize(&mut conn)?;
        }
    }
    Ok(conn)
}

pub fn secure_connection(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "trusted_schema", "OFF")?;
    conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    Ok(())
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
        [],
        |row| row.get(0),
    )?;
    if tables == 0 && version == 0 && app == 0 && allow_empty {
        return Ok(());
    }
    let expected_tables = if version < 2 {
        3
    } else if version == 2 {
        5
    } else {
        7
    };
    if tables != expected_tables || (version > 0 && app != APPLICATION_ID) {
        bail!(
            "Unrecognized or incomplete SwapCtx database; preserve it and restore a valid backup"
        );
    }
    if version >= 2 {
        validate_metadata(conn)?;
    }
    let executable_schema: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type IN ('trigger', 'view')",
        [],
        |r| r.get(0),
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
        conn.prepare(sql)
            .context("Incomplete SwapCtx schema; no records were modified")?;
    }
    for (table, columns) in [
        ("projects", &["id", "path", "created_at"][..]),
        (
            "rules",
            &["id", "project_id", "pattern", "reason", "created_at"][..],
        ),
        (
            "memories",
            &["id", "project_id", "kind", "content", "created_at"][..],
        ),
    ] {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, bool>(3)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        let actual = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        let invalid = actual.iter().zip(columns).enumerate().any(
            |(i, ((name, kind, required, pk), expected))| {
                if name.as_str() != *expected {
                    return true;
                }
                if i == 0 {
                    return kind != "INTEGER" || *pk != 1;
                }
                let expected_type = if name == "project_id" {
                    "INTEGER"
                } else {
                    "TEXT"
                };
                kind != expected_type || !*required || *pk != 0
            },
        );
        if actual.len() != columns.len() || invalid {
            bail!("Unrecognized columns in {table}; refusing to modify this database");
        }
    }
    for table in ["rules", "memories"] {
        let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list({table})"))?;
        let keys = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let expected = (
            "projects".to_owned(),
            "project_id".to_owned(),
            "id".to_owned(),
            "CASCADE".to_owned(),
        );
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
        [],
        |r| r.get(0),
    )?;
    if !unique_path {
        bail!("Missing unique project identity constraint");
    }
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
    if version(&tx)? < 2 {
        tx.execute_batch(include_str!("schema_v2.sql"))?;
    }
    if version(&tx)? < 3 {
        tx.execute_batch(include_str!("schema_v3.sql"))?;
    }
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

fn validate_metadata(conn: &Connection) -> Result<()> {
    let modern = version(conn)? >= 3;
    let memory_columns = if modern {
        &[
            "memory_id",
            "state",
            "memory_key",
            "expires_at",
            "reviewed_at",
            "superseded_by",
            "validity",
            "invalid_reason",
        ][..]
    } else {
        &[
            "memory_id",
            "state",
            "memory_key",
            "expires_at",
            "reviewed_at",
            "superseded_by",
        ][..]
    };
    for (table, columns) in [
        ("rule_policies", &["rule_id", "policy"][..]),
        ("memory_metadata", memory_columns),
    ] {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let actual = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, i64>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let invalid = actual
            .iter()
            .enumerate()
            .any(|(i, (name, kind, required, pk))| {
                let expected_type = if matches!(
                    name.as_str(),
                    "state" | "memory_key" | "policy" | "validity" | "invalid_reason"
                ) {
                    "TEXT"
                } else {
                    "INTEGER"
                };
                name != columns.get(i).copied().unwrap_or("")
                    || kind != expected_type
                    || *required != matches!(name.as_str(), "state" | "policy" | "validity")
                    || *pk != if i == 0 { 1 } else { 0 }
            });
        if actual.len() != columns.len() || invalid {
            bail!("Unrecognized metadata schema in {table}");
        }
    }
    if modern {
        for (table, columns) in [
            (
                "memory_origins",
                &["memory_id", "origin_id", "source", "actor", "recorded_at"][..],
            ),
            (
                "memory_events",
                &["id", "memory_id", "action", "detail", "recorded_at"][..],
            ),
        ] {
            let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let actual = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, bool>(3)?,
                        r.get::<_, i64>(5)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            if actual.len() != columns.len()
                || actual
                    .iter()
                    .enumerate()
                    .any(|(i, (name, kind, required, pk))| {
                        name != columns[i]
                            || kind
                                != if matches!(name.as_str(), "id" | "memory_id" | "recorded_at") {
                                    "INTEGER"
                                } else {
                                    "TEXT"
                                }
                            || *required != (i != 0)
                            || *pk != if i == 0 { 1 } else { 0 }
                    })
            {
                bail!("Unrecognized history schema in {table}");
            }
            let valid: bool=conn.query_row("SELECT count(*)=1 FROM pragma_foreign_key_list(?1) WHERE [table]='memories' AND [from]='memory_id' AND [to]='id' AND on_delete='CASCADE'",[table],|r|r.get(0))?;
            if !valid {
                bail!("Missing history isolation constraint");
            }
        }
        let unique: bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM pragma_index_list('memory_origins') i JOIN pragma_index_info(i.name) c WHERE i.[unique]=1 AND i.partial=0 GROUP BY i.name HAVING count(*)=1 AND max(c.name='origin_id')=1)",[],|r|r.get(0))?;
        if !unique {
            bail!("Missing origin identity constraint");
        }
    }
    // These tables are owned by SwapCtx; application code also enforces that
    // supersession stays within one project and always points to a new record.
    for (table, parent, column, delete) in [
        ("rule_policies", "rules", "rule_id", "CASCADE"),
        ("memory_metadata", "memories", "memory_id", "CASCADE"),
        ("memory_metadata", "memories", "superseded_by", "NO ACTION"),
    ] {
        let valid: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_list(?1)
             WHERE [table]=?2 AND [from]=?3 AND [to]='id' AND on_delete=?4)",
            params![table, parent, column, delete],
            |r| r.get(0),
        )?;
        if !valid {
            bail!("Missing metadata foreign key in {table}");
        }
    }
    Ok(())
}

pub fn diagnose(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA integrity_check")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    for row in rows {
        if row? != "ok" {
            bail!("SQLite integrity check failed; preserve the store and restore a backup");
        }
    }
    let mut fk = conn.prepare("PRAGMA foreign_key_check")?;
    if fk.query([])?.next()?.is_some() {
        bail!("Database has orphaned references; preserve it before attempting recovery");
    }
    if version(conn)? >= 2 {
        let invalid: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM memory_metadata d
             JOIN memories old ON old.id=d.memory_id
             LEFT JOIN memories replacement ON replacement.id=d.superseded_by
             WHERE d.state NOT IN ('active','archived','superseded')
                OR (d.state='superseded') != (d.superseded_by IS NOT NULL)
                OR (d.superseded_by IS NOT NULL AND replacement.project_id != old.project_id)
                OR (d.expires_at IS NOT NULL AND (typeof(d.expires_at)!='integer' OR d.expires_at <= 0)))
             OR EXISTS(SELECT 1 FROM rule_policies WHERE policy NOT IN ('deny','ask','audit'))",
            [],
            |r| r.get(0),
        )?;
        if invalid {
            bail!("Invalid lifecycle or policy metadata; no repair was attempted");
        }
        // Linear graph traversal; numeric IDs may change during portable import.
        let mut stmt = conn.prepare(
            "SELECT memory_id,superseded_by FROM memory_metadata WHERE superseded_by IS NOT NULL",
        )?;
        let links = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?
            .collect::<rusqlite::Result<std::collections::HashMap<_, _>>>()?;
        let mut done = std::collections::HashSet::new();
        for start in links.keys() {
            let mut path = std::collections::HashSet::new();
            let mut current = *start;
            while !done.contains(&current) {
                if !path.insert(current) {
                    bail!("Cyclic supersession history; no repair was attempted");
                }
                match links.get(&current) {
                    Some(next) => current = *next,
                    None => break,
                }
            }
            done.extend(path);
        }
    }
    if version(conn)? >= 3 {
        let invalid: bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM memories m LEFT JOIN memory_origins o ON o.memory_id=m.id WHERE o.memory_id IS NULL OR length(o.origin_id)!=32 OR o.origin_id GLOB '*[^0-9a-f]*' OR trim(o.source)='' OR trim(o.actor)='' OR typeof(o.recorded_at)!='integer' OR o.recorded_at<=0)
        OR EXISTS(SELECT 1 FROM memory_metadata WHERE validity NOT IN ('valid','invalid') OR (validity='invalid' AND (invalid_reason IS NULL OR trim(invalid_reason)='')))
        OR EXISTS(SELECT 1 FROM memory_events WHERE action NOT IN ('legacy','add','review','archive','supersede','invalidate','import') OR typeof(recorded_at)!='integer' OR recorded_at<=0)
        OR EXISTS(SELECT 1 FROM memories m WHERE NOT EXISTS(SELECT 1 FROM memory_events e WHERE e.memory_id=m.id))
        OR EXISTS(SELECT 1 FROM memory_metadata WHERE reviewed_at IS NOT NULL AND (typeof(reviewed_at)!='integer' OR reviewed_at<=0))",[],|r|r.get(0))?;
        if invalid {
            bail!("Invalid provenance or validity metadata; no repair was attempted");
        }
    }
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
        let id = conn
            .query_row(query, params![path_text(root)?], |row| row.get(0))
            .optional()?;
        if let Some(id) = id {
            return Ok((id, root.to_path_buf()));
        }
    }
    bail!("No registered project contains the current folder. Run 'swapctx init' at the project root.")
}

/// New knowledge/evidence commands need provenance; ordinary reads never migrate.
pub fn require_current(conn: &Connection) -> Result<()> {
    if version(conn)? != SCHEMA_VERSION {
        bail!("This command requires schema 3; back up the store, then run an explicit write command (such as init) to migrate; doctor is read-only");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_writer_survives_temporary_lock_beyond_five_seconds() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("store.db");
        let project = temp.path().canonicalize().unwrap();
        let mut holder = open(&path, true, true).unwrap();
        let mut queued = open(&path, false, true).unwrap();
        let lock = holder
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let (started, ready) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            started.send(()).unwrap();
            register(&mut queued, &project)
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        // Model a slow Windows commit while another already-open writer queues.
        // The existing five-second handler loses that write before release.
        std::thread::sleep(Duration::from_secs(6));
        lock.commit().unwrap();
        let result = writer.join().unwrap();
        assert!(result.is_ok(), "Queued writer failed: {result:?}");
        assert_eq!(
            holder
                .query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        validate(&holder, false).unwrap();
    }

    #[test]
    fn initialization_is_versioned_and_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../tests/fixtures/v0.sql"))
            .unwrap();
        conn.execute("INSERT INTO projects(path) VALUES ('example')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO memories(project_id,kind,content) VALUES (1,'requirement','keep')",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 0).unwrap();
        conn.pragma_update(None, "application_id", 0).unwrap();
        initialize(&mut conn).unwrap();
        initialize(&mut conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT content FROM memories", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "keep"
        );
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
    }

    #[test]
    fn rejects_foreign_and_future_schemas() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE other(data TEXT)", []).unwrap();
        assert!(initialize(&mut conn).is_err());
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type='table'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&mut conn).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(initialize(&mut conn).is_err());
    }

    #[test]
    fn rejects_unexpected_triggers_before_writing() {
        let mut conn = Connection::open_in_memory().unwrap();
        initialize(&mut conn).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER unexpected AFTER INSERT ON memories BEGIN DELETE FROM memories; END;",
        )
        .unwrap();
        assert!(initialize(&mut conn).is_err());
        assert!(validate(&conn, false).is_err());
    }

    #[test]
    fn migration_rollback_preserves_original_database() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../tests/fixtures/v0.sql"))
            .unwrap();
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)
            .unwrap();
        assert!(tx.execute("INSERT INTO missing VALUES (1)", []).is_err());
        drop(tx);
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        validate(&conn, false).unwrap();
    }

    #[test]
    fn failed_real_migration_preserves_schema_version_and_user_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../tests/fixtures/v0.sql"))
            .unwrap();
        conn.execute_batch("INSERT INTO projects(path) VALUES('project'); INSERT INTO memories(project_id,kind,content) VALUES(1,'requirement','keep'); CREATE INDEX rule_policies ON projects(path);").unwrap();
        assert!(initialize(&mut conn).is_err());
        assert_eq!(version(&conn).unwrap(), 0);
        assert_eq!(
            conn.query_row("SELECT content FROM memories", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "keep"
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type='table'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            3
        );
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
        assert_eq!(
            project(&conn, &root.join("nested/src")).unwrap().1,
            root.join("nested")
        );
        assert_eq!(project(&conn, &root.join("other")).unwrap().1, root);
    }
}
