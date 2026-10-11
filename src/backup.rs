//! Consistent SQLite snapshots, published only after verification; never overwrite.
use anyhow::{bail, Context, Result};
use rusqlite::{
    backup::{Backup, StepResult},
    Connection, OpenFlags,
};
use std::{
    fs::OpenOptions,
    path::Path,
    thread,
    time::{Duration, Instant},
};

pub fn snapshot(source: &Connection, destination: &Path) -> Result<()> {
    crate::paths::path_text(destination)?;
    if destination.try_exists()? {
        bail!("Backup destination already exists; nothing was overwritten");
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staged = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
    let mut target = Connection::open_with_flags(&staged, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    target.busy_timeout(Duration::from_secs(1))?;
    target.pragma_update(None, "foreign_keys", "ON")?;
    {
        let backup = Backup::new(source, &mut target)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match backup.step(256)? {
                StepResult::Done => break,
                StepResult::More | StepResult::Busy | StepResult::Locked => {
                    if Instant::now() >= deadline {
                        bail!("Backup timed out; no destination was published");
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                _ => bail!("Unsupported SQLite backup status"),
            }
        }
    }
    crate::storage::diagnose(&target)?;
    drop(target);
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&staged)?
        .sync_all()?;
    staged
        .persist_noclobber(destination)
        .map_err(|e| e.error)
        .context("Cannot publish verified backup; existing destination was not overwritten")?;
    Ok(())
}
