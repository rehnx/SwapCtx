//! Independently verify boundary rules, context completeness, and constraints without trusting agent claims.
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;
use std::{path::Path, process::Command};

use crate::{display::safe, paths, sensitive};

#[derive(Args, Debug)]
pub struct Options {
    /// Strict mode: fail if any warnings, expired records, unreviewed items, or unscanned files exist.
    #[arg(long)]
    pub strict: bool,
    /// Generate a deterministic offline canonical state fingerprint for CI or audit.
    #[arg(long)]
    pub attest: bool,
    /// Emit machine-readable JSON verification verdict.
    #[arg(long)]
    pub json: bool,
}

pub struct CheckResult {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
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

pub fn run(
    conn: &mut Connection,
    project: i64,
    cwd: &Path,
    root: &Path,
    options: Options,
) -> Result<u8> {
    let mut checks = Vec::new();
    let now = crate::memory::now()?;

    // 1. Storage & Lifecycle Integrity
    let integrity_check = match crate::storage::diagnose(conn) {
        Ok(_) => CheckResult {
            name: "Database Integrity",
            passed: true,
            detail: "SQLite PRAGMA ok, foreign keys ok, lifecycle metadata valid".into(),
        },
        Err(e) => CheckResult {
            name: "Database Integrity",
            passed: false,
            detail: format!("Integrity failure: {e:#}"),
        },
    };
    checks.push(integrity_check);

    // 2. Mandatory Context Completeness
    let request = crate::context::Request {
        protocol_version: 1,
        query: None,
        max_bytes: 7168,
        limit: 100,
        include_stale: false,
        include_conflicts: false,
        allow_unknown_provenance: false,
    };

    let context_check = match crate::context::packet(conn, cwd, &request) {
        Ok(raw_json) => {
            let (complete, req_returned, req_total, review_req, truncated): (
                bool,
                i64,
                i64,
                i64,
                bool,
            ) = conn.query_row(
                "SELECT
                        coalesce(json_extract(?1, '$.coverage.mandatory_context_complete'), 0),
                        coalesce(json_extract(?1, '$.coverage.requirements_returned'), 0),
                        coalesce(json_extract(?1, '$.coverage.active_valid_requirements'), 0),
                        coalesce(json_extract(?1, '$.coverage.review_required_records'), 0),
                        coalesce(json_extract(?1, '$.scan.truncated'), 0)",
                [&raw_json],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )?;

            // In strict mode, check for any expired records in project
            let expired_count: i64 = if options.strict && crate::storage::version(conn)? >= 2 {
                conn.query_row(
                    "SELECT count(*) FROM memory_metadata d
                     JOIN memories m ON m.id = d.memory_id
                     WHERE m.project_id = ?1 AND coalesce(d.state, 'active') = 'active'
                       AND d.expires_at IS NOT NULL AND d.expires_at <= ?2",
                    rusqlite::params![project, now],
                    |r| r.get(0),
                )
                .unwrap_or(0)
            } else {
                0
            };

            let passed = complete
                && req_returned == req_total
                && (!options.strict || (review_req == 0 && expired_count == 0))
                && !truncated;

            let detail = if passed {
                format!(
                    "{req_returned}/{req_total} mandatory requirements complete locally (model adherence unverified)"
                )
            } else if options.strict && (review_req > 0 || expired_count > 0) {
                format!(
                    "Strict mode failure: review_required={review_req}, expired_records={expired_count}"
                )
            } else {
                format!(
                    "{req_returned}/{req_total} requirements delivered, review_required={review_req}, complete={complete}"
                )
            };

            CheckResult {
                name: "Context Readiness",
                passed,
                detail,
            }
        }
        Err(e) => CheckResult {
            name: "Context Readiness",
            passed: false,
            detail: format!("Context packet generation failed: {e:#}"),
        },
    };
    checks.push(context_check);

    // 3. Worktree Boundary Compliance (Fail closed if git fails or not a git repository)
    let mut boundary_passed = true;
    let mut boundary_details = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT r.id, r.pattern, r.reason, coalesce(p.policy, 'deny')
         FROM rules r
         LEFT JOIN rule_policies p ON p.rule_id = r.id
         WHERE r.project_id = ?1
         ORDER BY r.id ASC",
    )?;
    let rules = stmt
        .query_map([project], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let git_status = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain=v1", "-uall"])
        .output();

    let mut unscanned_files_count = 0;
    let mut secret_passed = true;
    let mut secret_detail =
        String::from("No secret markers detected in active memories or worktree changes");

    match git_status {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if line.len() < 4 {
                    continue;
                }
                let raw_path = &line[3..];
                // Handle potential rename target: "R  orig -> new"
                let target_path = if let Some((_, dest)) = raw_path.split_once(" -> ") {
                    dest.trim()
                } else {
                    raw_path.trim()
                };
                let path_clean = target_path.trim_matches('"').replace('\\', "/");

                // Check rules against path
                for (id, pattern, reason, policy) in &rules {
                    if let Ok(matcher) = paths::rule_matcher(pattern) {
                        if matcher.is_match(&path_clean) {
                            if policy.eq_ignore_ascii_case("deny") {
                                boundary_passed = false;
                                boundary_details.push(format!(
                                    "File '{path_clean}' breaches rule #{id} [DENY: {reason}]"
                                ));
                            } else if options.strict && policy.eq_ignore_ascii_case("ask") {
                                boundary_passed = false;
                                boundary_details.push(format!(
                                    "File '{path_clean}' requires human approval under rule #{id} [ASK: {reason}]"
                                ));
                            }
                        }
                    }
                }

                // Check file content for secrets if modified and existing
                let full = root.join(&path_clean);
                if full.is_file() {
                    match std::fs::metadata(&full) {
                        Ok(meta) if meta.len() > 256 * 1024 => {
                            unscanned_files_count += 1;
                        }
                        Ok(_) => match std::fs::read_to_string(&full) {
                            Ok(content) => {
                                for line in content.lines() {
                                    if sensitive::guard(line, false).is_err() {
                                        secret_passed = false;
                                        secret_detail = format!(
                                            "Worktree file '{path_clean}' contains sensitive credential markers"
                                        );
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                unscanned_files_count += 1;
                            }
                        },
                        Err(_) => {
                            unscanned_files_count += 1;
                        }
                    }
                }
            }
        }
        Ok(out) => {
            // Git exited with error (fail closed)
            boundary_passed = false;
            let stderr_snippet = String::from_utf8_lossy(&out.stderr);
            boundary_details.push(format!(
                "Git evidence collection failed (status {}): {}",
                out.status,
                stderr_snippet.trim()
            ));
        }
        Err(e) => {
            // Git executable missing or error (fail closed)
            boundary_passed = false;
            boundary_details.push(format!("Git inspection unavailable (failed closed): {e}"));
        }
    }

    if options.strict && unscanned_files_count > 0 {
        boundary_passed = false;
        boundary_details.push(format!(
            "Strict mode failure: {unscanned_files_count} modified files were oversized or unreadable and could not be verified"
        ));
    }

    checks.push(CheckResult {
        name: "Boundary Policies",
        passed: boundary_passed,
        detail: if boundary_passed {
            "All modified worktree files conform to active boundary rules".into()
        } else {
            boundary_details.join("; ")
        },
    });

    // 4. Sensitive Credential & Secret Check (Active Memories)
    if secret_passed {
        let mut mem_stmt = conn.prepare(
            "SELECT m.id, m.content, m.kind
             FROM memories m
             LEFT JOIN memory_metadata d ON d.memory_id = m.id
             WHERE m.project_id = ?1 AND coalesce(d.state, 'active') = 'active' AND coalesce(d.validity, 'valid') = 'valid'",
        )?;
        let mut mem_rows = mem_stmt.query([project])?;
        while let Some(r) = mem_rows.next()? {
            let id: i64 = r.get(0)?;
            let content: String = r.get(1)?;
            let kind: String = r.get(2)?;
            if sensitive::guard(&content, false).is_err() || sensitive::guard(&kind, false).is_err()
            {
                secret_passed = false;
                secret_detail = format!("Active memory #{id} contains secret-like credentials");
                break;
            }
        }
    }

    if secret_passed && unscanned_files_count > 0 {
        secret_detail = format!(
            "No secrets detected in memories; {unscanned_files_count} oversized/binary file(s) skipped"
        );
    }

    checks.push(CheckResult {
        name: "Secret Safety Guard",
        passed: secret_passed,
        detail: secret_detail,
    });

    let all_passed = checks.iter().all(|c| c.passed);

    // Attestation fingerprint computation (Deterministic Canonical State Fingerprint)
    let attestation_hash = if options.attest || options.json {
        compute_canonical_state_fingerprint(conn, project, root, all_passed)?
    } else {
        String::new()
    };

    if options.json {
        render_json(conn, &checks, all_passed, &attestation_hash)?;
    } else {
        render_text(&checks, all_passed, options.attest, &attestation_hash)?;
    }

    if all_passed {
        Ok(0)
    } else {
        Ok(1)
    }
}

/// Computes a reproducible canonical state fingerprint across active memories, rules, git head and verdict.
fn compute_canonical_state_fingerprint(
    conn: &Connection,
    project: i64,
    root: &Path,
    passed: bool,
) -> Result<String> {
    let mut feed = Vec::new();
    feed.extend_from_slice(b"swapctx-canonical-v1\n");
    feed.extend_from_slice(if passed {
        b"verdict:pass\n"
    } else {
        b"verdict:fail\n"
    });

    // Sorted active memories
    let mut m_stmt = conn.prepare(
        "SELECT m.id, m.kind, m.content, coalesce(d.memory_key, ''), coalesce(d.state, 'active'), coalesce(d.validity, 'valid')
         FROM memories m
         LEFT JOIN memory_metadata d ON d.memory_id = m.id
         WHERE m.project_id = ?1
         ORDER BY m.id ASC",
    )?;
    let mut m_rows = m_stmt.query([project])?;
    let mut mem_count = 0i64;
    while let Some(r) = m_rows.next()? {
        let id: i64 = r.get(0)?;
        let kind: String = r.get(1)?;
        let content: String = r.get(2)?;
        let key: String = r.get(3)?;
        let state: String = r.get(4)?;
        let validity: String = r.get(5)?;
        let row_repr = format!("mem:{id}:{kind}:{key}:{state}:{validity}:{content}\n");
        feed.extend_from_slice(row_repr.as_bytes());
        mem_count += 1;
    }

    // Sorted active rules
    let mut r_stmt = conn.prepare(
        "SELECT r.id, r.pattern, r.reason, coalesce(p.policy, 'deny')
         FROM rules r
         LEFT JOIN rule_policies p ON p.rule_id = r.id
         WHERE r.project_id = ?1
         ORDER BY r.id ASC",
    )?;
    let mut r_rows = r_stmt.query([project])?;
    let mut rule_count = 0i64;
    while let Some(r) = r_rows.next()? {
        let id: i64 = r.get(0)?;
        let pattern: String = r.get(1)?;
        let reason: String = r.get(2)?;
        let policy: String = r.get(3)?;
        let row_repr = format!("rule:{id}:{pattern}:{policy}:{reason}\n");
        feed.extend_from_slice(row_repr.as_bytes());
        rule_count += 1;
    }

    // Include Git head if available for worktree binding
    let git_head = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output();
    if let Ok(out) = git_head {
        if out.status.success() {
            let commit = String::from_utf8_lossy(&out.stdout).trim().to_string();
            feed.extend_from_slice(format!("git_head:{commit}\n").as_bytes());
        }
    }

    let (h1, h2) = fnv1a128(&feed);
    let hash_manifest = format!(
        "attest:swapctx:v1:{}:{}:{}:{:016x}{:016x}",
        if passed { "pass" } else { "fail" },
        mem_count,
        rule_count,
        h1,
        h2
    );
    Ok(hash_manifest)
}

fn fnv1a128(bytes: &[u8]) -> (u64, u64) {
    let mut h1 = 0xcbf29ce484222325u64;
    let mut h2 = 0x84222325cbf29ce4u64;
    for &b in bytes {
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(0x100000001b3);
        h2 ^= (b as u64).rotate_left(3);
        h2 = h2.wrapping_mul(0x100000001b3);
    }
    (h1, h2)
}

fn render_text(
    checks: &[CheckResult],
    all_passed: bool,
    show_attest: bool,
    attestation_hash: &str,
) -> Result<()> {
    println!("SwapCtx Independent Constraint Verification\n");

    for check in checks {
        let mark = if check.passed { "[PASS]" } else { "[FAIL]" };
        println!("{mark} {}: {}", check.name, safe(&check.detail));
    }

    println!();
    if all_passed {
        println!("VERIFICATION VERDICT: PASS (all local constraints independently satisfied)");
        println!("Note: Context prepared locally; model receipt and adherence remain unverified.");
        if show_attest {
            println!("Attestation Manifest: {attestation_hash}");
        }
    } else {
        println!("VERIFICATION VERDICT: FAIL (one or more project constraints violated)");
    }

    Ok(())
}

fn render_json(
    conn: &Connection,
    checks: &[CheckResult],
    all_passed: bool,
    attestation_hash: &str,
) -> Result<()> {
    let checks_json = checks
        .iter()
        .map(|c| {
            format!(
                "{{\"name\":\"{}\",\"passed\":{},\"detail\":\"{}\"}}",
                json_escape(c.name),
                c.passed,
                json_escape(&c.detail)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let checks_str = format!("[{checks_json}]");

    let output: String = conn.query_row(
        "SELECT json_object(
            'protocol', 'swapctx.verify',
            'version', 1,
            'verdict', ?1,
            'all_passed', json(?2),
            'attestation', ?3,
            'checks', json(?4),
            'model_adherence', 'unverified'
        )",
        rusqlite::params![
            if all_passed { "PASS" } else { "FAIL" },
            if all_passed { "true" } else { "false" },
            attestation_hash,
            checks_str
        ],
        |r| r.get(0),
    )?;

    println!("{output}");
    Ok(())
}
