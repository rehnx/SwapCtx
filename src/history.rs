//! Self-reported provenance and transactional local history, not tamper evidence.
use anyhow::{bail, Result};
use rusqlite::{params, Connection};

pub fn origin(conn: &Connection, id: i64, source: &str, actor: &str) -> Result<()> {
    conn.execute("INSERT INTO memory_origins(memory_id,origin_id,source,actor,recorded_at) VALUES(?1,lower(hex(randomblob(16))),?2,?3,?4)", params![id,source,actor,crate::memory::now()?])?;
    Ok(())
}

pub fn event(conn: &Connection, id: i64, action: &str, detail: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO memory_events(memory_id,action,detail,recorded_at) VALUES(?1,?2,?3,?4)",
        params![id, action, detail, crate::memory::now()?],
    )?;
    Ok(())
}

pub fn identifier(conn: &Connection, id: i64) -> Result<String> {
    Ok(conn.query_row(
        "SELECT origin_id FROM memory_origins WHERE memory_id=?1",
        [id],
        |r| r.get(0),
    )?)
}

pub fn show(conn: &Connection, project: i64, id: i64) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM memories WHERE id=?1 AND project_id=?2)",
        params![id, project],
        |r| r.get(0),
    )?;
    if !exists {
        bail!("Memory record not found in this project");
    }
    if crate::storage::version(conn)? < 3 {
        println!("Origin and action history unavailable in this legacy schema");
        return Ok(());
    }
    let (origin, source, actor, time): (String, String, String, i64) = conn.query_row(
        "SELECT origin_id,source,actor,recorded_at FROM memory_origins WHERE memory_id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    println!(
        "Origin={} source={} actor={} registered_unix={time}",
        crate::sensitive::display(&origin, false),
        crate::sensitive::display(&source, false),
        crate::sensitive::display(&actor, false)
    );
    let mut stmt = conn.prepare(
        "SELECT action,detail,recorded_at FROM memory_events WHERE memory_id=?1 ORDER BY id",
    )?;
    let mut rows = stmt.query([id])?;
    while let Some(r) = rows.next()? {
        println!(
            "{} unix={} {}",
            crate::sensitive::display(&r.get::<_, String>(0)?, false),
            r.get::<_, i64>(2)?,
            crate::sensitive::display(&r.get::<_, String>(1)?, false)
        );
    }
    println!("Provenance is self-reported local evidence; history is not tamper-proof.");
    Ok(())
}
