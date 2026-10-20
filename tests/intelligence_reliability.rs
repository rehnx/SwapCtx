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

    // Invalidate it
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

    // 1. Two consecutive verify calls on identical state must produce IDENTICAL fingerprints
    let out1 = stdout(s.run(&["verify", "--attest"], 0));
    let out2 = stdout(s.run(&["verify", "--attest"], 0));

    let extract_hash = |out: &str| -> String {
        out.lines()
            .find(|l| l.starts_with("Attestation Manifest:"))
            .expect("manifest line")
            .split_whitespace()
            .last()
            .unwrap()
            .to_string()
    };

    let fp1 = extract_hash(&out1);
    let fp2 = extract_hash(&out2);
    assert_eq!(
        fp1, fp2,
        "Identical states must produce identical canonical fingerprints"
    );
    assert!(fp1.starts_with("attest:swapctx:v1:pass:1:0:"));

    // 2. Meaningful state change (adding a rule) MUST alter the fingerprint
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
        "State change must alter the canonical fingerprint"
    );
    assert!(fp3.starts_with("attest:swapctx:v1:pass:1:1:"));
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
    assert!(err_apply.contains("Invalid memory kind 'malicious_injected_kind'"));

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
fn test_handoff_export_preview_apply() {
    let s1 = Sandbox::new();
    s1.run(&["init"], 0);

    s1.run(
        &[
            "intent",
            "add",
            "Use transactional rollbacks for all billing operations",
            "--key",
            "billing:tx",
        ],
        0,
    );
    s1.run(
        &[
            "protect",
            "add",
            "--path",
            "billing/**",
            "--reason",
            "Financial compliance",
            "--policy",
            "deny",
        ],
        0,
    );

    let bundle_file = s1.project.join("billing-handoff.json");
    let bundle_str = bundle_file.to_str().unwrap();

    // Export handoff bundle
    let exp_out = stdout(s1.run(
        &[
            "handoff",
            "export",
            bundle_str,
            "--include-rules",
            "--target",
            "backend-agent",
        ],
        0,
    ));
    assert!(exp_out.contains("Created agent handoff bundle"));
    assert!(exp_out.contains("Target recipient:              backend-agent"));

    // In a new, separate agent sandbox (Agent B)
    let s2 = Sandbox::new();
    s2.run(&["init"], 0);

    // Preview handoff before applying
    let prev_out = stdout(s2.run(&["handoff", "preview", bundle_str], 0));
    assert!(prev_out.contains("Previewing Agent Knowledge Handoff Bundle"));
    assert!(prev_out.contains("Requirements: 1 approved entries"));
    assert!(prev_out.contains("Rules:        1 boundary rules"));
    assert!(prev_out.contains("billing operations"));

    // Apply handoff
    let apply_out = stdout(s2.run(&["handoff", "apply", bundle_str], 0));
    assert!(apply_out.contains("Successfully applied agent handoff bundle"));
    assert!(apply_out.contains("Imported requirements & intent: 1"));
    assert!(apply_out.contains("Imported boundary rules:       1"));

    // Verify Agent B now has the intent directive and boundary rule
    let list_b = stdout(s2.run(&["intent", "list"], 0));
    assert!(list_b.contains("billing operations"));
    assert!(list_b.contains("key=\"billing:tx\""));
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
    assert!(ver_out.contains("Attestation Manifest: attest:swapctx:v1:pass"));

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
