//! Read-only Git evidence, scoped to the selected registered project.
use anyhow::{bail, Context, Result};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
};

pub const SCAN_BYTES: usize = 256 * 1024;

pub fn bounded_file(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(SCAN_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn index_content(root: &Path, relative: &str) -> Result<Vec<u8>> {
    let mut child = git(root)
        .args([
            "show",
            "--no-ext-diff",
            "--no-textconv",
            &format!(":./{relative}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .context("Missing Git evidence pipe")?
        .take(SCAN_BYTES as u64 + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > SCAN_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        read?;
        return Ok(bytes);
    }
    if !child.wait()?.success() {
        bail!("Index evidence unavailable");
    }
    Ok(bytes)
}

pub fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_LITERAL_PATHSPECS", "1");
    command
}

pub fn changes(root: &Path) -> Result<Vec<crate::inspect::GitChange>> {
    let prefix = git(root).args(["rev-parse", "--show-prefix"]).output()?;
    if !prefix.status.success() {
        bail!("Git status inspection failed: project scope unavailable; failed closed");
    }
    let prefix = std::str::from_utf8(&prefix.stdout)?;
    let prefix = prefix.strip_suffix('\n').unwrap_or(prefix);
    let output = git(root)
        .args(["status", "--porcelain=v1", "-z", "-uall", "--", "."])
        .output()
        .context("Cannot collect Git status")?;
    if !output.status.success() {
        bail!("Git status evidence unavailable; inspection failed closed");
    }
    let mut changes = crate::inspect::parse_git_status_z(&output.stdout)?;
    for change in &mut changes {
        change.path = change
            .path
            .strip_prefix(prefix)
            .context("Git change crosses the registered project boundary")?
            .to_owned();
        if let Some(original) = &mut change.orig_path {
            *original = original
                .strip_prefix(prefix)
                .context("Git rename crosses the registered project boundary")?
                .to_owned();
        }
    }
    Ok(changes)
}

/// Resolve without reading file contents. Both lexical and resolved rule names matter.
pub fn targets(root: &Path, relative: &str) -> Result<Vec<String>> {
    crate::paths::git_target_paths(relative, root)
}

/// Never follow symlinks or junctions outside the project when reading evidence.
pub fn readable_file(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    targets(root, relative)?;
    let resolved = root.join(relative).canonicalize()?;
    if !resolved.is_file() {
        bail!("Worktree entry is not an ordinary file");
    }
    Ok(resolved)
}
