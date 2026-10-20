//! Inspect concurrent agent worktree changes against boundary policies without modifying worktrees.
use anyhow::{bail, Context, Result};
use clap::Args;
use rusqlite::Connection;
use std::{path::Path, process::Command};

use crate::{display::safe, paths, sensitive};

#[derive(Args, Debug)]
pub struct Options {
    /// Show detailed diff lines for touched protected files.
    #[arg(long)]
    pub diff: bool,
    /// Inspect a specific relative path instead of all git changes.
    #[arg(long)]
    pub path: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SensitiveScanResult {
    Clean,
    Detected,
    OversizedSkipped(u64),
    UnreadableSkipped,
}

#[derive(Debug)]
pub struct FileChange {
    pub path: String,
    pub status: String,
    pub matched_rules: Vec<RuleHit>,
    pub sensitive_scan: SensitiveScanResult,
}

#[derive(Debug, Clone)]
pub struct RuleHit {
    pub rule_id: i64,
    pub pattern: String,
    pub policy: String,
    pub reason: String,
}

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

pub fn run(conn: &Connection, project: i64, root: &Path, options: Options) -> Result<u8> {
    // Collect boundary rules from database
    let mut stmt = conn.prepare(
        "SELECT r.id, r.pattern, r.reason, coalesce(p.policy, 'deny')
         FROM rules r
         LEFT JOIN rule_policies p ON p.rule_id = r.id
         WHERE r.project_id = ?1
         ORDER BY r.id ASC",
    )?;
    let rules = stmt
        .query_map([project], |r| {
            Ok(RuleHit {
                rule_id: r.get(0)?,
                pattern: r.get(1)?,
                reason: r.get(2)?,
                policy: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let changes = collect_changes(root, options.path.as_deref())?;

    let mut audited_changes = Vec::new();
    let mut has_deny_violation = false;
    let mut has_sensitive_leak = false;

    for (file_rel, status) in changes {
        let mut matched = Vec::new();
        for rule in &rules {
            if let Ok(matcher) = paths::rule_matcher(&rule.pattern) {
                if matcher.is_match(&file_rel) {
                    if rule.policy.eq_ignore_ascii_case("deny") {
                        has_deny_violation = true;
                    }
                    matched.push(rule.clone());
                }
            }
        }

        // Check file content for sensitive tokens with honest coverage
        let sensitive_scan = check_file_sensitive(root, &file_rel, &status);
        if sensitive_scan == SensitiveScanResult::Detected {
            has_sensitive_leak = true;
        }

        audited_changes.push(FileChange {
            path: file_rel,
            status,
            matched_rules: matched,
            sensitive_scan,
        });
    }

    if options.json {
        render_json(
            conn,
            project,
            &audited_changes,
            has_deny_violation,
            has_sensitive_leak,
        )?;
    } else {
        render_text(
            root,
            &audited_changes,
            &rules,
            options.diff,
            options.path.as_deref(),
        )?;
    }

    if has_deny_violation || has_sensitive_leak {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn collect_changes(root: &Path, specific_path: Option<&str>) -> Result<Vec<(String, String)>> {
    let mut cmd = Command::new("git");
    cmd.current_dir(root)
        .args(["status", "--porcelain=v1", "-uall"]);
    if let Some(target) = specific_path {
        cmd.arg("--").arg(target);
    }

    let output = cmd
        .output()
        .context("Failed to execute git command. Ensure git is installed and in PATH.")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Git status inspection failed (exit code {}): {}",
            output.status,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let status = line[..2].trim().to_string();
        let raw_path = &line[3..];

        // Safe parsing of Git renames and quoted paths
        let target_path = if let Some((_, dest)) = raw_path.split_once(" -> ") {
            dest.trim()
        } else {
            raw_path.trim()
        };

        // Unquote Git-quoted filenames if wrapped in quotes
        let unquoted =
            if target_path.starts_with('"') && target_path.ends_with('"') && target_path.len() >= 2
            {
                &target_path[1..target_path.len() - 1]
            } else {
                target_path
            };

        let normalized = unquoted.replace('\\', "/");
        results.push((normalized, status));
    }

    Ok(results)
}

fn check_file_sensitive(root: &Path, rel_path: &str, status: &str) -> SensitiveScanResult {
    let full_path = root.join(rel_path);
    if status.contains('D') || !full_path.exists() {
        return SensitiveScanResult::Clean;
    }

    let metadata = match std::fs::metadata(&full_path) {
        Ok(m) => m,
        Err(_) => return SensitiveScanResult::UnreadableSkipped,
    };

    if metadata.len() > 256 * 1024 {
        return SensitiveScanResult::OversizedSkipped(metadata.len());
    }

    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            for line in content.lines() {
                if sensitive::guard(line, false).is_err() {
                    return SensitiveScanResult::Detected;
                }
            }
            SensitiveScanResult::Clean
        }
        Err(_) => SensitiveScanResult::UnreadableSkipped,
    }
}

fn render_text(
    root: &Path,
    changes: &[FileChange],
    rules: &[RuleHit],
    show_diff: bool,
    target_filter: Option<&str>,
) -> Result<()> {
    println!("SwapCtx Worktree Inspection — Concurrent Agent Changes\n");
    println!("Project root: {}", safe(paths::path_text(root)?));
    println!("Protected boundary rules configured: {}\n", rules.len());

    if changes.is_empty() {
        if let Some(target) = target_filter {
            println!(
                "No uncommitted changes detected by Git for path '{}'.",
                safe(target)
            );
        } else {
            println!("No uncommitted worktree changes detected by Git.");
        }
        println!("Status: CLEAN\n");
        return Ok(());
    }

    let mut violations = 0;
    let mut warnings = 0;
    let mut oversized = 0;
    let mut unreadable = 0;

    println!("Detected Worktree Changes ({} files):", changes.len());
    for change in changes {
        let status_desc = match change.status.as_str() {
            "M" | " M" | "M " => "modified",
            "??" => "untracked",
            "A" | "A " => "staged addition",
            "D" | " D" => "deleted",
            "R" | "R " => "renamed",
            s => s,
        };

        let badge = if change.matched_rules.iter().any(|r| r.policy == "deny") {
            violations += 1;
            " [VIOLATION: DENY]"
        } else if change.matched_rules.iter().any(|r| r.policy == "ask") {
            warnings += 1;
            " [REVIEW REQUIRED: ASK]"
        } else if !change.matched_rules.is_empty() {
            " [AUDIT]"
        } else {
            ""
        };

        let sensitive_note = match &change.sensitive_scan {
            SensitiveScanResult::Detected => " [SENSITIVE PATTERN DETECTED]",
            SensitiveScanResult::OversizedSkipped(_size) => {
                oversized += 1;
                " [SCAN SKIPPED: FILE > 256KB]"
            }
            SensitiveScanResult::UnreadableSkipped => {
                unreadable += 1;
                " [SCAN SKIPPED: UNREADABLE/BINARY]"
            }
            SensitiveScanResult::Clean => "",
        };

        println!(
            "  - {} ({}){}{}",
            safe(&change.path),
            status_desc,
            badge,
            sensitive_note
        );

        for rule in &change.matched_rules {
            println!(
                "      Rule #{}: policy={} pattern=\"{}\" reason=\"{}\"",
                rule.rule_id,
                rule.policy,
                safe(&rule.pattern),
                safe(&rule.reason)
            );
        }

        if show_diff && !change.matched_rules.is_empty() {
            // Safe diff preview for touched protected files
            let diff_output = Command::new("git")
                .current_dir(root)
                .args(["diff", "--", &change.path])
                .output();
            if let Ok(out) = diff_output {
                let diff_text = String::from_utf8_lossy(&out.stdout);
                for line in diff_text.lines().take(15) {
                    println!("        {}", safe(line));
                }
            }
        }
    }

    println!();
    if oversized > 0 || unreadable > 0 {
        println!(
            "Note: Honest coverage reporting — {} oversized and {} unreadable/binary file(s) were not scanned.",
            oversized, unreadable
        );
    }

    if violations > 0 {
        println!(
            "Inspection Result: VIOLATIONS DETECTED ({} deny policies breached)",
            violations
        );
        println!("Action: Agent modified denied files without authorization.");
    } else if warnings > 0 {
        println!(
            "Inspection Result: ATTENTION ({} files require explicit review)",
            warnings
        );
    } else {
        println!("Inspection Result: PASS (all modifications conform to active boundary rules)");
    }

    Ok(())
}

fn render_json(
    conn: &Connection,
    _project: i64,
    changes: &[FileChange],
    has_deny: bool,
    has_sensitive: bool,
) -> Result<()> {
    let mut files_json = Vec::new();
    for c in changes {
        let rules_arr = c
            .matched_rules
            .iter()
            .map(|r| {
                format!(
                    "{{\"id\":{},\"pattern\":\"{}\",\"policy\":\"{}\"}}",
                    r.rule_id,
                    json_escape(&r.pattern),
                    json_escape(&r.policy)
                )
            })
            .collect::<Vec<_>>()
            .join(",");

        let scan_status = match c.sensitive_scan {
            SensitiveScanResult::Clean => "clean",
            SensitiveScanResult::Detected => "detected",
            SensitiveScanResult::OversizedSkipped(_) => "oversized_skipped",
            SensitiveScanResult::UnreadableSkipped => "unreadable_skipped",
        };

        files_json.push(format!(
            "{{\"path\":\"{}\",\"status\":\"{}\",\"sensitive\":{},\"sensitive_status\":\"{}\",\"rules\":[{}]}}",
            json_escape(&c.path),
            json_escape(&c.status),
            c.sensitive_scan == SensitiveScanResult::Detected,
            scan_status,
            rules_arr
        ));
    }

    let files_str = format!("[{}]", files_json.join(","));
    let verdict = if has_deny || has_sensitive {
        "FAIL"
    } else {
        "PASS"
    };

    let output: String = conn.query_row(
        "SELECT json_object(
            'protocol', 'swapctx.inspect',
            'version', 1,
            'verdict', ?1,
            'violations_count', ?2,
            'sensitive_leak', json(?3),
            'files', json(?4)
        )",
        rusqlite::params![
            verdict,
            if has_deny { 1 } else { 0 },
            if has_sensitive { "true" } else { "false" },
            files_str
        ],
        |r| r.get(0),
    )?;

    println!("{output}");
    Ok(())
}
