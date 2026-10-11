use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};

pub fn path_text(path: &Path) -> Result<&str> {
    path.to_str().context("Paths must be valid Unicode; refusing a lossy project identity")
}

fn validate_segment(segment: &str) -> Result<()> {
    if segment.contains(':') || segment.starts_with(' ') || segment.ends_with(['.', ' ']) {
        bail!("Ambiguous Windows paths and alternate data streams are not supported");
    }
    let base = segment.split('.').next().unwrap_or("").trim_end_matches(' ').to_ascii_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ["COM", "LPT"].iter().any(|prefix| {
            base.strip_prefix(*prefix).is_some_and(|n| {
                matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
            })
        })
    {
        bail!("Windows device names are not supported");
    }
    Ok(())
}

pub fn rule_glob(pattern: &str) -> Result<Glob> {
    if pattern.trim().is_empty() || pattern.len() > 4096 || pattern.contains(['\\', ':'])
        || pattern.chars().any(char::is_control)
        || Path::new(pattern).is_absolute()
        || pattern.split('/').any(|s| s.is_empty() || s == "." || s == "..")
    {
        bail!("Use a nonempty project-relative forward-slash glob, such as 'src/auth/**'");
    }
    for segment in pattern.split('/') {
        validate_segment(segment)?;
    }
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .case_insensitive(cfg!(windows))
        .build()
        .context("Invalid protection glob")
}

fn resolve_missing(full: &Path) -> Result<PathBuf> {
    let mut ancestor = full.to_path_buf();
    let mut absent_parts = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(_) => break,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                absent_parts.push(ancestor.file_name().context("Unable to resolve target")?.to_os_string());
                if !ancestor.pop() {
                    bail!("Unable to resolve target");
                }
            }
            Err(e) => return Err(e).context("Cannot inspect target ancestor"),
        }
    }
    // Broken links, inaccessible ancestors and links outside the root fail closed.
    let mut resolved = ancestor.canonicalize().context("Cannot resolve target ancestor")?;
    if !absent_parts.is_empty() && !resolved.is_dir() {
        bail!("Target ancestor is not a directory");
    }
    for part in absent_parts.iter().rev() {
        resolved.push(part);
    }
    Ok(resolved)
}

fn relative_text(path: &Path, root: &Path) -> Result<String> {
    #[cfg(not(windows))]
    let relative = path.strip_prefix(root).context("Path is outside the registered project")?;
    // Windows canonical paths can retain caller spelling. Compare components,
    // never a string prefix (which would also accept sibling project names).
    #[cfg(windows)]
    let relative = {
        let mut parts = path.components();
        for expected in root.components() {
            let actual = parts.next().context("Path is outside the registered project")?;
            if !path_text(Path::new(actual.as_os_str()))?.eq_ignore_ascii_case(path_text(Path::new(expected.as_os_str()))?) {
                bail!("Path is outside the registered project");
            }
        }
        parts.as_path()
    };
    if relative.as_os_str().is_empty() {
        bail!("Provide a file or subdirectory, not the project root");
    }
    Ok(path_text(relative)?.replace('\\', "/"))
}

/// Check both the name supplied by the caller and its resolved destination.
/// This is a snapshot check, not a race-free authorization to modify a file.
pub fn target_paths(target: &str, cwd: &Path, root: &Path) -> Result<Vec<String>> {
    if target.trim().is_empty() || target.chars().any(char::is_control) {
        bail!("Target path must be nonempty and contain no control characters");
    }
    let provided = Path::new(target);
    for component in provided.components() {
        match component {
            Component::ParentDir => bail!("Parent-directory traversal ('..') is not allowed"),
            Component::Normal(segment) => {
                let segment = segment.to_str().context("Invalid Unicode path")?;
                validate_segment(segment)?;
                #[cfg(windows)]
                if segment.contains(['<', '>', '"', '|', '?', '*']) {
                    bail!("Check targets must be literal Windows paths, not wildcard expressions");
                }
            }
            _ => {}
        }
    }
    if provided.has_root() != provided.is_absolute()
        || (!provided.is_absolute() && provided.components().any(|p| matches!(p, Component::Prefix(_))))
    {
        bail!("Drive-relative and root-relative paths are not supported");
    }
    #[cfg(windows)]
    if provided.components().any(|p| matches!(p, Component::Prefix(prefix)
        if !matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::UNC(_, _))))
    {
        bail!("Windows device and verbatim path input is not supported");
    }
    let full = if provided.is_absolute() {
        // Canonicalize the volume/root representation used for the lexical name too.
        #[cfg(windows)]
        {
            let mut normalized = PathBuf::new();
            for part in provided.components() {
                match part {
                    Component::Prefix(_) | Component::RootDir => normalized.push(part.as_os_str()),
                    _ => break,
                }
            }
            let volume = normalized.canonicalize().context("Cannot resolve target volume")?;
            let tail: PathBuf = provided.components().filter_map(|part| match part {
                Component::Normal(s) => Some(s),
                _ => None,
            }).collect();
            volume.join(tail)
        }
        #[cfg(not(windows))]
        { provided.to_path_buf() }
    } else {
        cwd.join(provided)
    };
    let lexical = relative_text(&full, root)?;
    let resolved = relative_text(&resolve_missing(&full)?, root)?;
    let mut paths = vec![lexical];
    if paths[0] != resolved {
        paths.push(resolved);
    }
    Ok(paths)
}

pub fn matches(pattern: &str, targets: &[String]) -> Result<bool> {
    let matcher = rule_matcher(pattern)?;
    Ok(targets.iter().any(|target| matcher.is_match(target)))
}

pub fn rule_matcher(pattern: &str) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    builder.add(rule_glob(pattern)?);
    // A subtree rule also protects the subtree directory itself.
    if let Some(directory) = pattern.strip_suffix("/**") {
        builder.add(rule_glob(directory)?);
    }
    // Glob::compile_matcher internally expects compilation to succeed. The
    // fallible set builder preserves a normal error path for untrusted patterns.
    builder.build().context("Cannot compile protection rule")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ambiguous_patterns() {
        for pattern in ["", " ", "../x", "src\\x", "C:foo", "/x", "./x", "src//x", "NUL", "x.", "x ", "["] {
            assert!(rule_glob(pattern).is_err(), "{pattern:?}");
        }
    }

    #[test]
    fn wildcard_and_subtree_semantics() {
        assert!(matches("src/auth/**", &["src/auth".into()]).unwrap());
        assert!(matches("src/auth/**", &["src/auth/a/b.rs".into()]).unwrap());
        assert!(!matches("src/*.rs", &["src/a/b.rs".into()]).unwrap());
        assert_eq!(matches("src/auth/**", &["SRC/AUTH/x".into()]).unwrap(), cfg!(windows));
    }

    #[test]
    fn validates_existing_and_missing_targets() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::write(root.join("file"), "x").unwrap();
        assert_eq!(target_paths("new/deep/file", &root, &root).unwrap(), ["new/deep/file"]);
        assert!(target_paths("file/child", &root, &root).is_err());
        for path in ["", ".", "../outside", "a/../b", "file:stream", "NUL", " name", "name.", "name "] {
            assert!(target_paths(path, &root, &root).is_err(), "{path:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn checks_alias_and_destination_and_rejects_broken_or_external_links() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir(root.join("real")).unwrap();
        symlink(root.join("real"), root.join("alias")).unwrap();
        let paths = target_paths("alias/missing", &root, &root).unwrap();
        assert!(matches("alias/**", &paths).unwrap());
        assert!(matches("real/**", &paths).unwrap());
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), root.join("external")).unwrap();
        symlink(root.join("absent"), root.join("broken")).unwrap();
        assert!(target_paths("external/missing", &root, &root).is_err());
        assert!(target_paths("broken/child", &root, &root).is_err());
    }
}
