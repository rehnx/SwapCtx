//! Dedicated integration tests for SwapCtx Intelligence & Reliability Suite:
//! 1. Project Intent & Invariants (`swapctx intent`)
//! 2. Worktree & Agent Diff Inspection (`swapctx inspect`)
//! 3. Context Decision Explainability (`swapctx explain`)
//! 4. Approved Agent Knowledge Handoff (`swapctx handoff`)
//! 5. Independent Constraint Verification & Attestation (`swapctx verify`)

use rusqlite::Connection;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

struct Sandbox {
    _temp: TempDir,
    project: PathBuf,
    data: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        Self::with_git(true)
    }

    fn new_non_git() -> Self {
        Self::with_git(false)
    }

    fn with_git(init_git: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let data = temp.path().join("data");
        fs::create_dir(&project).unwrap();

        if init_git {
            let _ = Command::new("git")
                .current_dir(&project)
                .args(["init", "-b", "main"])
                .output();
            let _ = Command::new("git")
                .current_dir(&project)
                .args(["config", "user.name", "test-agent"])
                .output();
            let _ = Command::new("git")
                .current_dir(&project)
                .args(["config", "user.email", "agent@swapctx.test"])
                .output();
            let _ = Command::new("git")
                .current_dir(&project)
                .args(["commit", "--allow-empty", "-m", "initial commit"])
                .output();
        }

        Self {
            _temp: temp,
            project,
            data,
        }
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_swapctx"));
        cmd.current_dir(cwd);
        if !args.contains(&"--data-dir") {
            cmd.arg("--data-dir").arg(&self.data);
        }
        cmd.args(args);
        cmd
    }

    fn run(&self, args: &[&str], code: i32) -> Output {
        let output = self.command(&self.project, args).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{args:?}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn db(&self) -> Connection {
        Connection::open(self.data.join("swapctx.db")).unwrap()
    }
}

fn stdout(output: Output) -> String {
    String::from_utf8(output.stdout).unwrap()
}

fn stderr(output: Output) -> String {
    String::from_utf8(output.stderr).unwrap()
}

fn git(project: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(project)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn merged_cli_exposes_all_command_families_without_opening_a_store() {
    let s = Sandbox::new();
    for command in [
        "native", "codex", "context", "memory", "intent", "inspect", "explain", "handoff",
        "verify", "import", "export",
    ] {
        s.run(&[command, "--help"], 0);
    }
    assert!(!s.data.exists());
}

#[test]
fn staged_credentials_cannot_be_hidden_by_later_worktree_edits() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    fs::write(
        s.project.join("config.txt"),
        "api_key = AKIA1234567890123456",
    )
    .unwrap();
    git(&s.project, &["add", "config.txt"]);
    fs::write(s.project.join("config.txt"), "safe replacement").unwrap();
    let inspected = stdout(s.run(&["inspect", "--json"], 1));
    assert!(inspected.contains("\"sensitive\":true"));
    assert!(!inspected.contains("AKIA1234567890123456"));
    s.run(&["verify", "--strict"], 1);
    git(&s.project, &["add", "config.txt"]);
    s.run(&["inspect", "--json"], 0);
    s.run(&["verify", "--strict"], 0);
}

#[test]
fn verification_json_preserves_failed_checks_when_git_evidence_is_unavailable() {
    let s = Sandbox::new_non_git();
    s.run(&["init"], 0);
    let json = stdout(s.run(&["verify", "--json"], 1));
    let (passed, fingerprint): (bool, String) = s
        .db()
        .query_row(
            "SELECT json_extract(?1, '$.all_passed'), json_extract(?1, '$.attestation')",
            [&json],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(!passed);
    assert!(fingerprint.is_empty());
    assert!(json.contains("State Fingerprint"));
    assert!(json.contains("Boundary Policies"));
}

#[test]
fn verification_detects_secret_markers_in_optional_memory_provenance() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "ordinary optional note",
            "--source",
            "api_key = AKIA1234567890123456",
            "--allow-sensitive",
        ],
        0,
    );
    let json = stdout(s.run(&["verify", "--json"], 1));
    assert!(json.contains("Active memory #1 contains secret-like credentials"));
    assert!(!json.contains("AKIA1234567890123456"));
}

#[test]
fn nested_git_project_inspection_and_fingerprint_exclude_sibling_changes() {
    let s = Sandbox::new();
    let nested = s.project.join("nested");
    fs::create_dir(&nested).unwrap();
    let run = |args: &[&str], code| {
        let output = s.command(&nested, args).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout(output)
    };
    run(&["init"], 0);
    run(
        &[
            "protect",
            "add",
            "--path",
            "blocked.txt",
            "--reason",
            "review",
            "--policy",
            "deny",
        ],
        0,
    );
    fs::write(nested.join("blocked.txt"), "nested data").unwrap();
    fs::write(s.project.join("sibling.txt"), "sibling data").unwrap();
    let inspected = run(&["inspect", "--json"], 1);
    assert!(inspected.contains("blocked.txt"));
    assert!(!inspected.contains("nested/blocked"));
    assert!(!inspected.contains("sibling"));
    run(&["verify", "--strict"], 1);
    fs::remove_file(nested.join("blocked.txt")).unwrap();
    let before = run(&["verify", "--json"], 0);
    fs::write(s.project.join("sibling.txt"), "different sibling data").unwrap();
    let after = run(&["verify", "--json"], 0);
    assert_eq!(
        before, after,
        "sibling changes must not affect project evidence"
    );
}

#[test]
fn explain_path_json_redacts_sensitive_content_and_metadata() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let secret = "auth api_key = AKIA1234567890123456";
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--allow-sensitive",
            secret,
        ],
        0,
    );
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "auth/**",
            "--reason",
            secret,
            "--allow-sensitive",
        ],
        0,
    );
    for args in [
        vec!["explain", "--path", "auth/login.rs", "--json"],
        vec!["explain", "--path", "auth/login.rs"],
    ] {
        let output = stdout(s.run(&args, 0));
        assert!(!output.contains("AKIA1234567890123456"));
        assert!(output.contains("potential sensitive value hidden"));
    }
    s.run(&["explain", "--path", "../outside", "--json"], 1);
}

#[test]
fn handoff_rejects_structural_ambiguity_and_oversize_without_writes() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let bundle = s._temp.path().join("incoming.json");
    for content in [
        r#"{"protocol":"swapctx.handoff","version":1}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"memories":{},"rules":[]}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"memories":[],"rules":[],"items_count":2}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"memories":[true],"rules":[]}"#,
    ] {
        fs::write(&bundle, content).unwrap();
        s.run(&["handoff", "preview", bundle.to_str().unwrap()], 1);
        s.run(&["handoff", "apply", bundle.to_str().unwrap()], 1);
    }
    fs::write(&bundle, vec![b' '; 16 * 1024 * 1024 + 1]).unwrap();
    assert!(stderr(s.run(&["handoff", "apply", bundle.to_str().unwrap()], 1)).contains("16 MiB"));
    let count: i64 = s
        .db()
        .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn imported_intent_remains_mandatory_and_blocks_unsafe_context() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let bundle = s._temp.path().join("intent.json");
    fs::write(&bundle, r#"{"protocol":"swapctx.handoff","version":1,"memories":[{"kind":"intent","content":"Keep authentication","key":"auth"}],"rules":[]}"#).unwrap();
    s.run(&["handoff", "apply", bundle.to_str().unwrap()], 0);
    let context = stdout(s.run(&["context"], 0));
    let requirements: i64 = s
        .db()
        .query_row(
            "SELECT json_extract(?1, '$.coverage.active_valid_requirements')",
            [context],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(requirements, 1);
    s.run(
        &["intent", "add", "Different authentication", "--key", "auth"],
        0,
    );
    s.run(&["verify", "--strict"], 1);
}

#[test]
fn handoff_decisions_and_notes_are_explicit_fresh_and_project_scoped() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["intent", "add", "Keep public interfaces"], 0);
    s.run(&["memory", "add", "--kind", "decision", "Use SQLite"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "note",
            "Next investigate latency",
        ],
        0,
    );
    s.run(
        &["memory", "add", "--kind", "note", "Obsolete implementation"],
        0,
    );
    s.db()
        .execute(
            "UPDATE memory_metadata SET expires_at=1 WHERE memory_id=4",
            [],
        )
        .unwrap();
    let default = s._temp.path().join("default.json");
    let all = s._temp.path().join("all.json");
    s.run(&["handoff", "export", default.to_str().unwrap()], 0);
    let baseline = fs::read_to_string(default).unwrap();
    assert!(baseline.contains("Keep public interfaces"));
    assert!(!baseline.contains("Use SQLite"));
    assert!(!baseline.contains("Next investigate"));
    s.run(
        &[
            "handoff",
            "export",
            all.to_str().unwrap(),
            "--include-decisions",
            "--include-notes",
        ],
        0,
    );
    let expanded = fs::read_to_string(&all).unwrap();
    assert!(expanded.contains("Use SQLite"));
    assert!(expanded.contains("Next investigate"));
    assert!(!expanded.contains("Obsolete implementation"));
    let recipient = s.project.join("recipient");
    fs::create_dir(&recipient).unwrap();
    for args in [
        vec!["init"],
        vec!["handoff", "apply", all.to_str().unwrap()],
    ] {
        let result = s.command(&recipient, &args).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let count: i64 = s
        .db()
        .query_row(
            "SELECT count(*) FROM memories WHERE project_id=2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 3);
}

#[test]
fn legacy_reads_preserve_store_and_write_migration_preserves_requirements() {
    for version in [0, 1, 2] {
        let s = Sandbox::new();
        fs::create_dir(&s.data).unwrap();
        let db = s.db();
        db.execute_batch(include_str!("fixtures/v0.sql")).unwrap();
        if version == 2 {
            db.execute_batch(include_str!("../src/schema_v2.sql"))
                .unwrap();
        }
        if version > 0 {
            db.pragma_update(None, "application_id", 0x53574358)
                .unwrap();
        }
        db.pragma_update(None, "user_version", version).unwrap();
        db.execute(
            "INSERT INTO projects(id, path) VALUES(1, ?1)",
            [s.project.canonicalize().unwrap().to_str().unwrap()],
        )
        .unwrap();
        db.execute("INSERT INTO memories(id,project_id,kind,content) VALUES(1,1,'REQUIREMENT','Preserve legacy knowledge')", []).unwrap();
        drop(db);
        let before = fs::read(s.data.join("swapctx.db")).unwrap();
        assert!(stdout(s.run(&["intent", "list"], 0)).contains("Preserve legacy knowledge"));
        assert!(stdout(s.run(&["memory", "audit"], 0)).contains("BLOCKED"));
        for args in [
            vec!["inspect"],
            vec!["verify", "--json"],
            vec!["explain", "--path", "file"],
            vec!["handoff", "export", "unused.json"],
        ] {
            assert!(stderr(s.run(&args, 1)).contains("requires schema 3"));
        }
        assert_eq!(before, fs::read(s.data.join("swapctx.db")).unwrap());
        s.run(&["intent", "pin", "1"], 0);
        assert_eq!(
            s.db()
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert!(stdout(s.run(&["memory", "get", "1"], 0)).contains("Preserve legacy knowledge"));
    }
}

#[cfg(unix)]
#[test]
fn inspection_does_not_read_external_symlinks_and_checks_internal_alias_rules() {
    use std::os::unix::fs::symlink;
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let outside = s._temp.path().join("private.txt");
    fs::write(&outside, "api_key = AKIA1234567890123456").unwrap();
    symlink(&outside, s.project.join("external.txt")).unwrap();
    let output = stdout(s.run(&["inspect", "--json"], 0));
    assert!(output.contains("unreadable_skipped"));
    assert!(!output.contains("AKIA1234567890123456"));
    assert!(!output.contains("\"sensitive\":true"));
    s.run(&["verify", "--strict"], 1);
    fs::remove_file(s.project.join("external.txt")).unwrap();
    fs::write(s.project.join("protected.txt"), "internal").unwrap();
    symlink(s.project.join("protected.txt"), s.project.join("alias.txt")).unwrap();
    git(&s.project, &["add", "protected.txt"]);
    git(&s.project, &["commit", "-m", "baseline"]);
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "protected.txt",
            "--reason",
            "keep",
        ],
        0,
    );
    s.run(&["inspect"], 1);
    s.run(&["verify"], 1);
}

#[cfg(unix)]
#[test]
fn porcelain_newlines_are_preserved_and_non_unicode_fails_closed() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let s = Sandbox::new();
    s.run(&["init"], 0);
    fs::write(s.project.join("line\nbreak.txt"), "text").unwrap();
    let json = stdout(s.run(&["inspect", "--json"], 0));
    assert!(json.contains("line\\nbreak.txt"));
    s.run(&["verify", "--strict"], 0);
    fs::write(s.project.join(OsString::from_vec(vec![b'x', 0xff])), "text").unwrap();
    s.run(&["inspect", "--json"], 1);
    s.run(&["verify", "--strict"], 1);
}

#[test]
fn test_intent_lifecycle_and_pinning() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // 1. Add authoritative project intent
    let add_out = stdout(s.run(
        &[
            "intent",
            "add",
            "All database queries must use prepared statements",
            "--key",
            "arch:sql",
            "--actor",
            "lead-architect",
        ],
        0,
    ));
    assert!(add_out.contains("Preserved project intent directive #1"));

    // 2. List intent directives
    let list_out = stdout(s.run(&["intent", "list"], 0));
    assert!(list_out.contains("#1 [INVARIANT]"));
    assert!(list_out.contains("key=\"arch:sql\""));
    assert!(list_out.contains("[by lead-architect via project-intent]"));
    assert!(list_out.contains("prepared statements"));

    // 3. Add a temporary requirement with expiry and pin it to permanent invariant
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--expires-in-days",
            "7",
            "Preserve backward compatibility with v1 API clients",
        ],
        0,
    );

    let pin_out = stdout(s.run(&["intent", "pin", "2"], 0));
    assert!(
        pin_out.contains("Pinned memory #2 as permanent project intent directive (expiry cleared)")
    );

    // Verify expiry was cleared in DB
    let expires: Option<i64> = s
        .db()
        .query_row(
            "SELECT expires_at FROM memory_metadata WHERE memory_id = 2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(expires.is_none());

    // 4. Retire an intent directive
    let retire_out = stdout(s.run(
        &[
            "intent",
            "retire",
            "1",
            "--reason",
            "Migrated to ORM layer with built-in parameterization",
        ],
        0,
    ));
    assert!(retire_out.contains("Retired project intent directive #1"));

    // Active intent list now shows only #2
    let updated_list = stdout(s.run(&["intent", "list"], 0));
    assert!(!updated_list.contains("#1 [INVARIANT]"));
    assert!(updated_list.contains("#2 [INVARIANT]"));
}

#[test]
fn test_intent_lifecycle_guards_reject_invalid_transitions() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Add requirement
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "Must validate TLS certificates",
        ],
        0,
    );

    // Add a note
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "note",
            "Consider caching parsed certificates",
        ],
        0,
    );

    // Attempting to pin a 'note' as an intent directive MUST FAIL (intent-kind restriction)
    let err_pin_note = stderr(s.run(&["intent", "pin", "2"], 1));
    assert!(err_pin_note.contains("only 'requirement' records can be promoted"));

    // Attempting to retire a 'note' via intent retire MUST FAIL
    let err_retire_note = stderr(s.run(&["intent", "retire", "2", "--reason", "Not needed"], 1));
    assert!(err_retire_note.contains("only project intent / requirement directives can be retired"));

    // Invalidate requirement #1
    s.run(
        &[
            "memory",
            "invalidate",
            "1",
            "--reason",
            "Deprecated by zero-trust mesh",
        ],
        0,
    );

    // Attempt to pin the invalidated requirement -> MUST FAIL
    let err_pin = stderr(s.run(&["intent", "pin", "1"], 1));
    assert!(err_pin.contains("marked invalid"));

    // Attempt to retire an already invalidated directive -> MUST FAIL
    let err_retire = stderr(s.run(&["intent", "retire", "1", "--reason", "Retiring again"], 1));
    assert!(err_retire.contains("already retired"));
}

#[test]
fn test_inspect_worktree_boundary_rules() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Add boundary protection rules
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "src/database/**",
            "--reason",
            "Database migrations require human architect review",
            "--policy",
            "deny",
        ],
        0,
    );
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "config/*",
            "--reason",
            "Review configuration changes",
            "--policy",
            "ask",
        ],
        0,
    );

    // Clean inspection on specific path that has no changes
    let clean_out = stdout(s.run(&["inspect", "--path", "src/utils.rs"], 0));
    assert!(clean_out.contains("CLEAN"));

    // Create a modified file matching DENY rule: must fail with exit code 1
    let db_schema_dir = s.project.join("src").join("database");
    fs::create_dir_all(&db_schema_dir).unwrap();
    fs::write(db_schema_dir.join("schema.rs"), "// new schema changes").unwrap();

    let deny_out = stdout(s.run(&["inspect"], 1));
    assert!(deny_out.contains("[VIOLATION: DENY]"));
    assert!(deny_out.contains("Rule #1: policy=deny"));
    assert!(deny_out.contains("VIOLATIONS DETECTED"));

    // Test JSON output mode
    let json_out = stdout(s.run(&["inspect", "--json"], 1));
    let db = s.db();
    let verdict: String = db
        .query_row("SELECT json_extract(?1, '$.verdict')", [&json_out], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(verdict, "FAIL");
}

#[test]
fn test_nul_delimited_git_status_and_renames() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Add protection rule on internal auth paths
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "internal/auth/**",
            "--reason",
            "Protected authentication core",
            "--policy",
            "deny",
        ],
        0,
    );

    // Commit a file under internal/auth
    let auth_dir = s.project.join("internal").join("auth");
    fs::create_dir_all(&auth_dir).unwrap();
    fs::write(auth_dir.join("token.rs"), "// token engine").unwrap();

    Command::new("git")
        .current_dir(&s.project)
        .args(["add", "internal/auth/token.rs"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(&s.project)
        .args(["commit", "-m", "add auth token"])
        .output()
        .unwrap();

    // Now rename protected file to an unprotected location via git mv
    let public_dir = s.project.join("public");
    fs::create_dir_all(&public_dir).unwrap();
    Command::new("git")
        .current_dir(&s.project)
        .args(["mv", "internal/auth/token.rs", "public/token.rs"])
        .output()
        .unwrap();

    // Inspect: must catch the violation via orig_path even though destination is 'public/'
    let inspect_out = stdout(s.run(&["inspect"], 1));
    assert!(inspect_out.contains("[VIOLATION: DENY]"));
    assert!(inspect_out.contains("renamed from internal/auth/token.rs"));
    assert!(inspect_out.contains("Rule #1: policy=deny"));

    // Test file with spaces in directory and filename
    let spaced_dir = s.project.join("my spaced folder");
    fs::create_dir_all(&spaced_dir).unwrap();
    fs::write(spaced_dir.join("my spaced file.rs"), "// spaced").unwrap();

    let spaced_out = stdout(s.run(
        &["inspect", "--path", "my spaced folder/my spaced file.rs"],
        0,
    ));
    assert!(spaced_out.contains("my spaced folder/my spaced file.rs"));

    // Test directory pathspec without discarding nested changes
    fs::write(spaced_dir.join("nested_a.rs"), "// a").unwrap();
    fs::write(spaced_dir.join("nested_b.rs"), "// b").unwrap();
    let dir_out = stdout(s.run(&["inspect", "--path", "my spaced folder"], 0));
    assert!(dir_out.contains("my spaced folder/nested_a.rs"));
    assert!(dir_out.contains("my spaced folder/nested_b.rs"));
    assert!(dir_out.contains("my spaced folder/my spaced file.rs"));
}

#[test]
fn test_secret_safe_diff_preview() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Protect config files with an 'ask' policy
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "config/**",
            "--reason",
            "Configuration audit",
            "--policy",
            "ask",
        ],
        0,
    );

    // Commit baseline config
    let config_dir = s.project.join("config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("app.env"), "PORT=8080\n").unwrap();

    Command::new("git")
        .current_dir(&s.project)
        .args(["add", "config/app.env"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(&s.project)
        .args(["commit", "-m", "baseline config"])
        .output()
        .unwrap();

    // Agent modifies file adding an AWS secret key
    fs::write(
        config_dir.join("app.env"),
        "PORT=8080\nAPI_KEY = AKIAIOSFODNN7EXAMPLE\n",
    )
    .unwrap();

    // Inspect with --diff flag (fails with exit code 1 because sensitive token was detected in file)
    let diff_out = stdout(s.run(&["inspect", "--diff"], 1));
    // Verify plaintext secret was REDACTED in diff preview!
    assert!(!diff_out.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(diff_out.contains("[potential sensitive value hidden]"));
    assert!(diff_out.contains("Secret-safe preview active"));
}

#[test]
fn test_git_unavailable_or_non_repo_fails_closed() {
    let s = Sandbox::new_non_git();
    s.run(&["init"], 0);

    // In a non-git directory, inspect must fail closed with exit code 1
    let inspect_err = stderr(s.run(&["inspect"], 1));
    assert!(inspect_err.contains("Git status inspection failed"));

    // In a non-git directory, verify must fail closed with exit code 1
    let verify_out = stdout(s.run(&["verify"], 1));
    assert!(verify_out.contains("[FAIL] Boundary Policies: Git evidence collection failed"));
    assert!(verify_out.contains("VERIFICATION VERDICT: FAIL"));
}

#[test]
fn test_state_fingerprint_determinism_and_sensitivity() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    s.run(
        &[
            "intent",
            "add",
            "Zero trust authentication required",
            "--key",
            "security:auth",
        ],
        0,
    );

    let extract_hash = |out: &str| -> String {
        out.lines()
            .find(|l| l.starts_with("Attestation Manifest:"))
            .expect("manifest line")
            .split_whitespace()
            .last()
            .unwrap()
            .to_string()
    };

    // 1. Two consecutive verify calls on identical state must produce IDENTICAL fingerprints
    let out1 = stdout(s.run(&["verify", "--attest"], 0));
    let out2 = stdout(s.run(&["verify", "--attest"], 0));
    let fp1 = extract_hash(&out1);
    let fp2 = extract_hash(&out2);
    assert_eq!(
        fp1, fp2,
        "Identical states must produce identical canonical fingerprints"
    );
    assert!(fp1.starts_with("attest:swapctx:v2:sha256:pass:1:0:"));

    // 2. Meaningful DB change (adding a rule) MUST alter the fingerprint
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "secret/**",
            "--reason",
            "Keep secrets isolated",
            "--policy",
            "deny",
        ],
        0,
    );

    let out3 = stdout(s.run(&["verify", "--attest"], 0));
    let fp3 = extract_hash(&out3);
    assert_ne!(
        fp1, fp3,
        "Rule state change must alter the canonical fingerprint"
    );
    assert!(fp3.starts_with("attest:swapctx:v2:sha256:pass:1:1:"));

    // 3. Worktree change (uncommitted modified file) MUST alter the fingerprint!
    fs::write(
        s.project.join("dirty.txt"),
        "uncommitted worktree modification 1",
    )
    .unwrap();
    let out4 = stdout(s.run(&["verify", "--attest"], 0));
    let fp4 = extract_hash(&out4);
    assert_ne!(
        fp3, fp4,
        "Uncommitted worktree changes must alter the canonical state fingerprint"
    );

    // 4. Content edit to an already-modified file with UNCHANGED git status MUST alter the fingerprint!
    fs::write(
        s.project.join("dirty.txt"),
        "uncommitted worktree modification 2 - altered file content",
    )
    .unwrap();
    let out4_b = stdout(s.run(&["verify", "--attest"], 0));
    let fp4_b = extract_hash(&out4_b);
    assert_ne!(
        fp4, fp4_b,
        "Editing content of an already modified file must alter the canonical fingerprint"
    );

    // 5. Memory metadata change (updating expiry) MUST alter the fingerprint!
    s.db()
        .execute(
            "UPDATE memory_metadata SET expires_at = 2000000000 WHERE memory_id = 1",
            [],
        )
        .unwrap();
    let out5 = stdout(s.run(&["verify", "--attest"], 0));
    let fp5 = extract_hash(&out5);
    assert_ne!(
        fp4_b, fp5,
        "Memory lifecycle/expiry update must alter the canonical state fingerprint"
    );
}

#[test]
fn test_handoff_conflict_detection_and_structural_matching() {
    let s1 = Sandbox::new();
    s1.run(&["init"], 0);

    // Add an approved requirement
    s1.run(
        &[
            "intent",
            "add",
            "Primary database is PostgreSQL 16",
            "--key",
            "db:primary",
        ],
        0,
    );

    let bundle_file = s1.project.join("db-handoff.json");
    let bundle_str = bundle_file.to_str().unwrap();

    // Export clean handoff bundle
    s1.run(
        &["handoff", "export", bundle_str, "--target", "db-agent"],
        0,
    );

    // Sandbox 2 has a competing active record sharing key "db:primary" with DIFFERENT content
    let s2 = Sandbox::new();
    s2.run(&["init"], 0);
    s2.run(
        &[
            "intent",
            "add",
            "Primary database is MySQL 8",
            "--key",
            "db:primary",
        ],
        0,
    );

    // 1. Previewing bundle in s2 MUST detect and flag the key conflict
    let prev_out = stdout(s2.run(&["handoff", "preview", bundle_str], 0));
    assert!(prev_out.contains("[CONFLICT: key \"db:primary\" competes with existing #1]"));
    assert!(prev_out.contains("Competing key conflicts detected:                 1"));

    // 2. Applying bundle in s2 MUST fail and reject atomically due to key conflict!
    let err_apply = stderr(s2.run(&["handoff", "apply", bundle_str], 1));
    assert!(err_apply.contains("Handoff rejected due to key conflict"));
    assert!(err_apply.contains("incoming key competes with active record #1"));

    // Verify atomic rollback in s2: database still only has 1 memory (the original MySQL 8)
    let count: i64 = s2
        .db()
        .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let content: String = s2
        .db()
        .query_row("SELECT content FROM memories WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(content, "Primary database is MySQL 8");

    // 3. Applying identical bundle on s1 (exact structural match) succeeds and skips duplicate
    let apply_s1 = stdout(s1.run(&["handoff", "apply", bundle_str], 0));
    assert!(apply_s1.contains("Imported requirements & intent: 0"));
    assert!(apply_s1.contains("Structurally matched (already present): 1"));

    // 4. Exporting from a project with active conflicts must EXCLUDE conflicted records
    // In s2, add a second competing requirement on key "db:primary" so it has an internal conflict
    s2.run(
        &[
            "intent",
            "add",
            "Primary database is Aurora",
            "--key",
            "db:primary",
        ],
        0,
    );
    let conflicted_export = s2.project.join("conflicted-export.json");
    let exp_conf_out = stdout(s2.run(
        &["handoff", "export", conflicted_export.to_str().unwrap()],
        0,
    ));
    assert!(exp_conf_out.contains("Eligible knowledge records:    0"));
}

#[test]
fn test_handoff_internal_bundle_conflict_and_rule_collision() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // 1. Internal duplicate key conflict inside the bundle itself
    let internal_conflict_bundle = s.project.join("internal-conflict.json");
    fs::write(
        &internal_conflict_bundle,
        r#"{
            "protocol": "swapctx.handoff",
            "version": 1,
            "target": "agent",
            "items_count": 2,
            "rules_count": 0,
            "memories": [
                {"id": 1, "kind": "requirement", "content": "Use Redis for caching", "key": "cache:engine"},
                {"id": 2, "kind": "requirement", "content": "Use Memcached for caching", "key": "cache:engine"}
            ],
            "rules": []
        }"#,
    )
    .unwrap();

    let err_internal = stderr(s.run(
        &[
            "handoff",
            "apply",
            internal_conflict_bundle.to_str().unwrap(),
        ],
        1,
    ));
    assert!(err_internal.contains("Bundle contains internal key conflict"));
    assert!(!err_internal.contains("cache:engine"));

    // DB must remain clean (0 memories)
    let count: i64 = s
        .db()
        .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);

    // 2. Existing rule pattern conflict with differing policy or reason
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "migrations/**",
            "--reason",
            "Human approval required for schema changes",
            "--policy",
            "ask",
        ],
        0,
    );

    let rule_conflict_bundle = s.project.join("rule-conflict.json");
    fs::write(
        &rule_conflict_bundle,
        r#"{
            "protocol": "swapctx.handoff",
            "version": 1,
            "target": "agent",
            "items_count": 0,
            "rules_count": 1,
            "memories": [],
            "rules": [
                {"pattern": "migrations/**", "reason": "Automated migrations allowed", "policy": "deny"}
            ]
        }"#,
    )
    .unwrap();

    // Preview flags rule conflict
    let prev_rule = stdout(s.run(
        &["handoff", "preview", rule_conflict_bundle.to_str().unwrap()],
        0,
    ));
    assert!(prev_rule.contains("[CONFLICT: pattern collides with differing policy/reason]"));

    // Apply rejects rule pattern collision atomically
    let err_rule = stderr(s.run(
        &["handoff", "apply", rule_conflict_bundle.to_str().unwrap()],
        1,
    ));
    assert!(err_rule.contains("Handoff rejected due to rule conflict"));
    assert!(err_rule.contains("conflicts with existing rule #1"));

    // 3. Exact matching rule in bundle is accepted and deduplicated
    let rule_match_bundle = s.project.join("rule-match.json");
    fs::write(
        &rule_match_bundle,
        r#"{
            "protocol": "swapctx.handoff",
            "version": 1,
            "target": "agent",
            "items_count": 0,
            "rules_count": 1,
            "memories": [],
            "rules": [
                {"pattern": "migrations/**", "reason": "Human approval required for schema changes", "policy": "ask"}
            ]
        }"#,
    )
    .unwrap();

    let ok_rule = stdout(s.run(
        &["handoff", "apply", rule_match_bundle.to_str().unwrap()],
        0,
    ));
    assert!(ok_rule.contains("Imported boundary rules:       0"));
}

#[test]
fn test_handoff_malformed_input_and_atomic_rollback() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Create a malformed handoff bundle with an invalid memory kind
    let bad_bundle = s.project.join("malformed-handoff.json");
    fs::write(
        &bad_bundle,
        r#"{
            "protocol": "swapctx.handoff",
            "version": 1,
            "target": "agent",
            "items_count": 2,
            "rules_count": 0,
            "memories": [
                {"id": 1, "kind": "requirement", "content": "Valid requirement 1"},
                {"id": 2, "kind": "malicious_injected_kind", "content": "Exploit attempt"}
            ],
            "rules": []
        }"#,
    )
    .unwrap();

    // Applying malformed bundle must fail
    let err_apply = stderr(s.run(&["handoff", "apply", bad_bundle.to_str().unwrap()], 1));
    assert!(err_apply.contains("Invalid memory kind"));

    // Atomic rollback check: Database MUST have 0 memories (neither record was committed!)
    let count: i64 = s
        .db()
        .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 0,
        "Failed handoff must roll back transaction atomically"
    );

    // Test refusal to overwrite existing file in export
    let existing_file = s.project.join("already-exists.json");
    fs::write(&existing_file, "existing").unwrap();
    let err_export = stderr(s.run(&["handoff", "export", existing_file.to_str().unwrap()], 1));
    assert!(err_export.contains("already exists or symlink race detected"));
}

#[test]
fn test_json_escaping_adversarial_multiline_unicode_controls() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Adversarial text containing newlines, quotes, backslashes, tabs, and Unicode
    let adversarial = "Line 1: \"quoted\"\nLine 2: \\back\\slash\r\nLine 3: \tTabbed & 🚀 Unicode";
    s.run(&["intent", "add", adversarial, "--key", "adv:text"], 0);

    // 1. Verify JSON output
    let ver_json = stdout(s.run(&["verify", "--json"], 0));
    let db = s.db();
    let protocol: String = db
        .query_row("SELECT json_extract(?1, '$.protocol')", [&ver_json], |r| {
            r.get(0)
        })
        .expect("verify JSON must parse cleanly with SQLite json_extract");
    assert_eq!(protocol, "swapctx.verify");

    // 2. Explain JSON output
    let exp_json = stdout(s.run(&["explain", "--id", "1", "--json"], 0));
    let exp_protocol: String = db
        .query_row("SELECT json_extract(?1, '$.protocol')", [&exp_json], |r| {
            r.get(0)
        })
        .expect("explain JSON must parse cleanly with SQLite json_extract");
    assert_eq!(exp_protocol, "swapctx.explain.record");

    // 3. Inspect JSON output
    let ins_json = stdout(s.run(&["inspect", "--json"], 0));
    let ins_protocol: String = db
        .query_row("SELECT json_extract(?1, '$.protocol')", [&ins_json], |r| {
            r.get(0)
        })
        .expect("inspect JSON must parse cleanly with SQLite json_extract");
    assert_eq!(ins_protocol, "swapctx.inspect");
}

#[test]
fn test_strict_mode_verification() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Add a memory record and manually set its expiry in past
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "note",
            "Temporary implementation note",
        ],
        0,
    );
    s.db()
        .execute(
            "UPDATE memory_metadata SET expires_at = 1 WHERE memory_id = 1",
            [],
        )
        .unwrap();

    // In normal mode, expired notes don't block context readiness
    // But in --strict mode, expired records must cause verification failure!
    let strict_out = stdout(s.run(&["verify", "--strict"], 1));
    assert!(strict_out.contains("[FAIL] Context Readiness"));
    assert!(strict_out.contains("Strict mode failure"));
    assert!(strict_out.contains("VERIFICATION VERDICT: FAIL"));
}

#[test]
fn test_explain_context_delivery_decisions() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Add two competing records sharing key 'auth'
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--key",
            "auth",
            "Use Session cookies",
        ],
        0,
    );
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--key",
            "auth",
            "Use JWT Bearer tokens",
        ],
        0,
    );

    // Explain record #1: should report OMITTED due to conflict on key 'auth'
    let exp1 = stdout(s.run(&["explain", "--id", "1"], 0));
    assert!(exp1.contains("OMITTED FROM CONTEXT"));
    assert!(exp1.contains("Conflicted explicit key \"auth\""));
    assert!(exp1.contains("Suggested fix:"));

    // Resolve conflict by invalidating #2
    s.run(
        &[
            "memory",
            "invalidate",
            "2",
            "--reason",
            "Replaced by cookie standard",
        ],
        0,
    );

    // Explain record #1 again: should now be DELIVERED
    let exp1_after = stdout(s.run(&["explain", "--id", "1"], 0));
    assert!(exp1_after.contains("DELIVERED IN CONTEXT"));

    // Regression test: prove unrelated IDs and substring tricks cannot produce false inclusion.
    // Record #1 exists and is in the packet.
    // Add memory #3 whose content contains the substring `"id":"999"`, but record #999 does not exist.
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "Text mentioning dummy payload: {\"id\":\"999\"}",
        ],
        0,
    );
    // Explaining non-existent record #999 must fail with record not found, not report DELIVERED.
    let err_999 = stderr(s.run(&["explain", "--id", "999"], 1));
    assert!(err_999.contains("Memory record not found in this project"));

    // Add record #4 with ID 10+ (e.g. #4 contains "id": "1", but #4 itself has ID 4).
    // Now add an expired note with ID 5 whose text contains `"id":"1"`.
    // Explaining record #5 must report OMITTED (expired), never falsely claiming DELIVERED due to substring match on record #1.
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "note",
            "Referencing upstream record \"id\":\"1\" in audit notes",
        ],
        0,
    );
    s.db()
        .execute(
            "UPDATE memory_metadata SET expires_at = 1 WHERE memory_id = 4",
            [],
        )
        .unwrap();
    let exp_note = stdout(s.run(&["explain", "--id", "4"], 0));
    assert!(exp_note.contains("OMITTED FROM CONTEXT"));
    assert!(exp_note.contains("Expired (stale)"));

    // Test Path Context Lens
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "src/auth/**",
            "--reason",
            "Authentication logic",
            "--policy",
            "deny",
        ],
        0,
    );
    let path_lens = stdout(s.run(&["explain", "--path", "src/auth/service.rs"], 0));
    assert!(path_lens.contains("Path Context Lens"));
    assert!(path_lens.contains("Rule #1 [DENY]"));
    assert!(path_lens.contains("Relevant Project Requirements & Intent"));
    assert!(path_lens.contains("Session cookies"));
}

#[test]
fn test_verify_independent_constraints() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Add a valid requirement
    s.run(
        &[
            "intent",
            "add",
            "Follow semantic versioning strictly",
            "--key",
            "release:semver",
        ],
        0,
    );

    // Verification on clean project: must pass
    let ver_out = stdout(s.run(&["verify", "--attest"], 0));
    assert!(ver_out.contains("[PASS] Database Integrity"));
    assert!(ver_out.contains("[PASS] Context Readiness"));
    assert!(ver_out.contains("[PASS] Boundary Policies"));
    assert!(ver_out.contains("[PASS] Secret Safety Guard"));
    assert!(ver_out.contains("VERIFICATION VERDICT: PASS"));
    assert!(ver_out.contains("Attestation Manifest: attest:swapctx:v2:sha256:pass"));

    // Test JSON verification mode
    let json_ver = stdout(s.run(&["verify", "--json"], 0));
    let db = s.db();
    let verdict: String = db
        .query_row("SELECT json_extract(?1, '$.verdict')", [&json_ver], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(verdict, "PASS");

    // Add a secret marker into a memory: must trigger verification failure
    s.run(
        &[
            "memory",
            "add",
            "--allow-sensitive",
            "api_key = AKIA1234567890123456 in production config",
        ],
        0,
    );

    let fail_out = stdout(s.run(&["verify"], 1));
    assert!(fail_out.contains("[FAIL] Secret Safety Guard"));
    assert!(fail_out.contains("VERIFICATION VERDICT: FAIL"));
}

fn attestation(s: &Sandbox) -> String {
    let output = stdout(s.run(&["verify", "--json"], 0));
    Connection::open_in_memory()
        .unwrap()
        .query_row("SELECT json_extract(?1, '$.attestation')", [&output], |r| {
            r.get(0)
        })
        .unwrap()
}

#[test]
fn regression_fingerprint_field_boundaries() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["intent", "add", "Retain audit records"], 0);
    let db = s.db();
    db.execute(
        "UPDATE memory_origins SET source='alpha:beta', actor='gamma'",
        [],
    )
    .unwrap();
    let first = attestation(&s);
    db.execute(
        "UPDATE memory_origins SET source='alpha', actor='beta:gamma'",
        [],
    )
    .unwrap();
    let second = attestation(&s);
    assert_ne!(first, second);
    assert_eq!(second, attestation(&s));
    let digest = second.rsplit(':').next().unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn regression_fingerprint_binds_index_and_raw_worktree() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let file = s.project.join("file.txt");
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(&s.project)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    // Index mutations are confined to this disposable test fixture.
    git(&["config", "core.autocrlf", "true"]);
    fs::write(&file, "staged one\n").unwrap();
    git(&["add", "file.txt"]);
    fs::write(&file, "working tree\n").unwrap();
    let first = attestation(&s);
    fs::write(&file, "staged two\n").unwrap();
    git(&["add", "file.txt"]);
    fs::write(&file, "working tree\n").unwrap();
    let second = attestation(&s);
    assert_ne!(
        first, second,
        "same status and worktree, different staged content"
    );
    fs::write(&file, "working tree\r\n").unwrap();
    assert_ne!(
        second,
        attestation(&s),
        "raw bytes must bind despite Git normalization"
    );
}

#[test]
fn regression_handoff_rules_round_trip_and_exact_conflict() {
    let source = Sandbox::new();
    source.run(&["init"], 0);
    source.run(
        &["intent", "add", "Use prepared queries", "--key", "database"],
        0,
    );
    source.run(
        &[
            "protect",
            "add",
            "--path",
            "migrations/**",
            "--reason",
            "Require review",
            "--policy",
            "ask",
        ],
        0,
    );
    let bundle = source.project.join("roundtrip.json");
    let bundle = bundle.to_str().unwrap();
    source.run(&["handoff", "export", bundle, "--include-rules"], 0);
    let target = Sandbox::new();
    target.run(&["init"], 0);
    assert!(stdout(target.run(&["handoff", "preview", bundle], 0)).contains("[NEW]"));
    let result = stdout(target.run(&["handoff", "apply", bundle], 0));
    assert!(result.contains("Imported boundary rules:       1"));
    assert!(result.contains("Imported requirements & intent: 1"));
    let db = target.db();
    let rule: (String, String, String) = db
        .query_row(
            "SELECT pattern, reason, policy FROM rules JOIN rule_policies ON rule_id=rules.id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        rule,
        (
            "migrations/**".into(),
            "Require review".into(),
            "ask".into()
        )
    );
    assert!(stdout(target.run(&["handoff", "apply", bundle], 0))
        .contains("Structurally matched (already present): 1"));
    target.run(
        &[
            "intent",
            "add",
            "Use a competing approach",
            "--key",
            "database",
        ],
        0,
    );
    assert!(stdout(target.run(&["handoff", "preview", bundle], 0)).contains("[CONFLICT:"));
    assert!(stderr(target.run(&["handoff", "apply", bundle], 1))
        .contains("Handoff rejected due to key conflict"));
    let counts: (i64, i64) = db
        .query_row(
            "SELECT (SELECT count(*) FROM memories), (SELECT count(*) FROM rules)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (2, 1));
}

#[test]
fn regression_secret_provenance_exports_and_conflict_diagnostics() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["intent", "add", "Retain records"], 0);
    let secret = "password=regression-credential";
    for column in ["source", "actor"] {
        s.db().execute(&format!("UPDATE memory_origins SET source='project-intent', actor='architect', {column}=?1"), [secret]).unwrap();
        for args in [vec!["intent", "list"], vec!["explain", "--id", "1"]] {
            let out = stdout(s.run(&args, 0));
            assert!(!out.contains(secret));
        }
        let bundle = s.project.join(format!("{column}.json"));
        s.run(&["handoff", "export", bundle.to_str().unwrap()], 0);
        let content = fs::read_to_string(bundle).unwrap();
        assert!(!content.contains(secret));
        let count: i64 = s
            .db()
            .query_row(
                "SELECT json_array_length(?1, '$.memories')",
                [&content],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "secure/**",
            "--reason",
            "Review",
            "--policy",
            "ask",
        ],
        0,
    );
    s.db()
        .execute("UPDATE rules SET reason=?1", [secret])
        .unwrap();
    let bundle = s.project.join("conflict.json");
    fs::write(&bundle, r#"{"protocol":"swapctx.handoff","version":1,"memories":[],"rules":[{"pattern":"secure/**","reason":"Different","policy":"deny"}]}"#).unwrap();
    let out = stderr(s.run(&["handoff", "apply", bundle.to_str().unwrap()], 1));
    assert!(out.contains("rule conflict"));
    assert!(!out.contains(secret));
    let db = s.db();
    db.execute("UPDATE rules SET reason='Review'", []).unwrap();
    // Exercise defensive export against a malformed legacy database.
    db.pragma_update(None, "ignore_check_constraints", true)
        .unwrap();
    db.execute("UPDATE rule_policies SET policy=?1", [secret])
        .unwrap();
    let export = s.project.join("policy.json");
    s.run(
        &[
            "handoff",
            "export",
            export.to_str().unwrap(),
            "--include-rules",
        ],
        0,
    );
    assert!(!fs::read_to_string(export).unwrap().contains(secret));
}

#[test]
fn regression_sensitive_key_conflicts_remain_redacted_with_override() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let secret = "password=regression-key";
    s.run(
        &[
            "intent",
            "add",
            "Existing",
            "--key",
            secret,
            "--allow-sensitive",
        ],
        0,
    );
    let bundle = s.project.join("key-conflict.json");
    fs::write(&bundle, format!(r#"{{"protocol":"swapctx.handoff","version":1,"target":"{secret}","memories":[{{"id":1,"kind":"intent","key":"{secret}","content":"Incoming"}}],"rules":[]}}"#)).unwrap();
    let preview = stdout(s.run(&["handoff", "preview", bundle.to_str().unwrap()], 0));
    assert!(preview.contains("CONFLICT"));
    assert!(!preview.contains(secret));
    let error = stderr(s.run(
        &[
            "handoff",
            "apply",
            bundle.to_str().unwrap(),
            "--allow-sensitive",
        ],
        1,
    ));
    assert!(error.contains("key conflict"));
    assert!(!error.contains(secret));
    s.run(
        &[
            "intent",
            "add",
            "Competing",
            "--key",
            secret,
            "--allow-sensitive",
        ],
        0,
    );
    for args in [
        vec!["explain", "--id", "1"],
        vec!["explain", "--id", "1", "--json"],
    ] {
        assert!(!stdout(s.run(&args, 0)).contains(secret));
    }
    fs::write(&bundle, format!(r#"{{"protocol":"swapctx.handoff","version":1,"memories":[{{"kind":"intent","key":"{secret}","content":"First"}},{{"kind":"intent","key":"{secret}","content":"Second"}}],"rules":[]}}"#)).unwrap();
    let fresh = Sandbox::new();
    fresh.run(&["init"], 0);
    let error = stderr(fresh.run(
        &[
            "handoff",
            "apply",
            bundle.to_str().unwrap(),
            "--allow-sensitive",
        ],
        1,
    ));
    assert!(error.contains("internal key conflict"));
    assert!(!error.contains(secret));
    let count: i64 = fresh
        .db()
        .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn inspection_text_reports_secrets_as_failure_and_json_counts_denied_files() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    fs::write(s.project.join("one.txt"), "api_key = AKIA1234567890123456").unwrap();
    let text = stdout(s.run(&["inspect"], 1));
    assert!(text.contains("Inspection Result: FAIL"));
    assert!(!text.contains("Inspection Result: PASS"));
    assert!(!text.contains("AKIA1234567890123456"));
    fs::write(s.project.join("two.txt"), "ordinary data").unwrap();
    s.run(
        &["protect", "add", "--path", "*.txt", "--reason", "review"],
        0,
    );
    let json = stdout(s.run(&["inspect", "--json"], 1));
    let count: i64 = s
        .db()
        .query_row(
            "SELECT json_extract(?1, '$.violations_count')",
            [json],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn handoff_same_key_different_kind_cannot_create_mandatory_conflicts() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "note",
            "--key",
            "shared",
            "Same wording",
        ],
        0,
    );
    let bundle = s._temp.path().join("kind-conflict.json");
    fs::write(&bundle, r#"{"protocol":"swapctx.handoff","version":1,"memories":[{"id":1,"kind":"requirement","content":"Same wording","key":"shared"}],"rules":[]}"#).unwrap();
    let preview = stdout(s.run(&["handoff", "preview", bundle.to_str().unwrap()], 0));
    assert!(preview.contains("[CONFLICT:"));
    s.run(&["handoff", "apply", bundle.to_str().unwrap()], 1);
    let count: i64 = s
        .db()
        .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn handoff_preview_accepts_optional_ids_and_normalizes_imported_kinds() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &["intent", "add", "Preserve interfaces", "--key", "compat"],
        0,
    );
    let bundle = s._temp.path().join("normalized.json");
    fs::write(&bundle, r#"{"protocol":"swapctx.handoff","version":1,"memories":[{"kind":"INTENT","content":"Preserve interfaces","key":"compat"}],"rules":[]}"#).unwrap();
    let preview = stdout(s.run(&["handoff", "preview", bundle.to_str().unwrap()], 0));
    assert!(preview.contains("[EXISTS:"));
    assert!(!preview.contains("[CONFLICT:"));
    let applied = stdout(s.run(&["handoff", "apply", bundle.to_str().unwrap()], 0));
    assert!(applied.contains("Structurally matched (already present): 1"));
}

#[test]
fn handoff_rejects_duplicate_json_members_and_invalid_field_types() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let bundle = s._temp.path().join("ambiguous.json");
    for content in [
        r#"{"protocol":"swapctx.handoff","version":1,"memories":[],"memories":[],"rules":[]}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"memories":[{"kind":"note","content":"First","content":"Second"}],"rules":[]}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"target":[],"memories":[],"rules":[]}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"items_count":null,"memories":[],"rules":[]}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"memories":[{"kind":"note","content":42}],"rules":[]}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"memories":[{"kind":"unsupported","content":"text"}],"rules":[]}"#,
        r#"{"protocol":"swapctx.handoff","version":1,"memories":[],"rules":[{"pattern":"[","reason":"test","policy":"deny"}]}"#,
    ] {
        fs::write(&bundle, content).unwrap();
        s.run(&["handoff", "preview", bundle.to_str().unwrap()], 1);
        s.run(&["handoff", "apply", bundle.to_str().unwrap()], 1);
    }
    assert_eq!(
        s.db()
            .query_row("SELECT count(*) FROM memories", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn handoff_preview_counts_entries_without_declared_counts_and_deduplicates() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let bundle = s._temp.path().join("duplicates.json");
    fs::write(&bundle, r#"{"protocol":"swapctx.handoff","version":1,"memories":[{"kind":"intent","content":"Preserve interfaces"},{"kind":"REQUIREMENT","content":"Preserve interfaces"}],"rules":[]}"#).unwrap();
    let preview = stdout(s.run(&["handoff", "preview", bundle.to_str().unwrap()], 0));
    assert!(preview.contains("Requirements: 2 approved entries"));
    assert!(preview.contains("New records ready to import:                      1"));
    assert!(preview.contains("Structurally identical records (already present): 1"));
    let before = fs::read(s.data.join("swapctx.db")).unwrap();
    s.run(&["handoff", "preview", bundle.to_str().unwrap()], 0);
    assert_eq!(before, fs::read(s.data.join("swapctx.db")).unwrap());
    let applied = stdout(s.run(&["handoff", "apply", bundle.to_str().unwrap()], 0));
    assert!(applied.contains("Imported requirements & intent: 1"));
    assert!(applied.contains("Structurally matched (already present): 1"));
}

#[test]
fn strict_verification_ignores_expiry_on_invalidated_requirements() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["intent", "add", "Old requirement"], 0);
    s.db()
        .execute("UPDATE memory_metadata SET expires_at=1000", [])
        .unwrap();
    s.run(&["memory", "invalidate", "1", "--reason", "Obsolete"], 0);
    s.run(&["verify", "--strict"], 0);
}
