//! Dedicated integration tests for `swapctx memory audit`.
//! Uses isolated disposable sandboxes and verifies deterministic outputs,
//! zero database mutation, and consistency with `swapctx context`.

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
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let data = temp.path().join("data");
        fs::create_dir(&project).unwrap();
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

#[test]
fn audit_empty_project_is_healthy_and_consistent_with_context() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Memory Health — Current Project"));
    assert!(text.contains("Active records: 0"));
    assert!(text.contains("Valid requirements: 0"));
    assert!(text.contains("Expired records: 0"));
    assert!(text.contains("Conflicting keys: 0"));
    assert!(text.contains("Review recommended: 0"));
    assert!(text.contains("Native context readiness: READY"));
    assert!(text.contains("Mandatory requirements: 0/0 delivered"));
    assert!(text.contains("Suggested action"));

    // Check JSON output mode
    let json_text = stdout(s.run(&["memory", "audit", "--json"], 0));
    let db = s.db();
    let readiness: String = db
        .query_row(
            "SELECT json_extract(?1, '$.readiness')",
            [&json_text],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(readiness, "READY");

    // Compare with swapctx context
    let ctx = stdout(s.run(&["context"], 0));
    let ctx_complete: bool = db
        .query_row(
            "SELECT json_extract(?1, '$.coverage.mandatory_context_complete')",
            [&ctx],
            |r| r.get(0),
        )
        .unwrap();
    assert!(ctx_complete);
}

#[test]
fn audit_valid_requirements_and_boundary_rules() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--source",
            "spec:v1",
            "--actor",
            "alice",
            "Preserve atomic commits",
        ],
        0,
    );
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "*.secret",
            "--reason",
            "Confidential",
        ],
        0,
    );

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Active records: 1"));
    assert!(text.contains("Valid requirements: 1"));
    assert!(text.contains("Expired records: 0"));
    assert!(text.contains("Conflicting keys: 0"));
    assert!(text.contains("Review recommended: 0"));
    assert!(text.contains("Native context readiness: READY"));
    assert!(text.contains("Mandatory requirements: 1/1 delivered"));
    assert!(text.contains("Boundary rules: 1/1 delivered"));

    // Confirm swapctx context also succeeds and reports complete
    let ctx = stdout(s.run(&["context"], 0));
    let db = s.db();
    let complete: bool = db
        .query_row(
            "SELECT json_extract(?1, '$.coverage.mandatory_context_complete')",
            [&ctx],
            |r| r.get(0),
        )
        .unwrap();
    assert!(complete);
}

#[test]
fn audit_expired_records_and_requirements_diagnostics() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--key",
            "auth_req",
            "--expires-in-days",
            "1",
            "Preserve auth token lifetime",
        ],
        0,
    );
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "note",
            "--expires-in-days",
            "1",
            "Temporary observation",
        ],
        0,
    );

    // Set expiry in the past deterministically
    s.db()
        .execute("UPDATE memory_metadata SET expires_at = 1000", [])
        .unwrap();

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Expired records: 2"));
    assert!(text.contains("(requirements: 1)"));
    assert!(text.contains("Review recommended: 2"));
    assert!(text.contains("Native context readiness: NEEDS ATTENTION"));
    assert!(text.contains("Exclusions from Native Context:"));
    assert!(text.contains("expired (stale)"));
    assert!(text.contains("Suggested action"));
    assert!(text.contains("Active requirement #1 is expired"));

    // Confirm that swapctx context omits expired records and reports mandatory incomplete
    let ctx = stdout(s.run(&["context"], 0));
    let db = s.db();
    let complete: bool = db
        .query_row(
            "SELECT json_extract(?1, '$.coverage.mandatory_context_complete')",
            [&ctx],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!complete);

    // Renew the requirement
    s.run(&["memory", "review", "1", "--expires-in-days", "30"], 0);
    // Archive the note
    s.run(&["memory", "archive", "2"], 0);

    let updated = stdout(s.run(&["memory", "audit"], 0));
    assert!(updated.contains("Expired records: 0"));
    assert!(updated.contains("Archived: 1"));
    assert!(updated.contains("Native context readiness: READY"));
}

#[test]
fn audit_conflicting_keys_and_resolution_via_supersession() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--key",
            "authentication",
            "Use basic HTTP auth",
        ],
        0,
    );
    s.run(
        &[
            "memory",
            "add",
            "--key",
            "authentication",
            "Use OAuth 2.0 with PKCE",
        ],
        0,
    );

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Conflicting keys: 1"));
    assert!(text.contains("(competing records: 2)"));
    assert!(text.contains("Review recommended: 2"));
    assert!(text.contains("Native context readiness: NEEDS ATTENTION"));
    assert!(text.contains(
        "Review the conflicting records associated with key \"authentication\" (records #1, #2)"
    ));

    // Supersede record 1 with a new official standard
    s.run(
        &[
            "memory",
            "supersede",
            "1",
            "Use OpenID Connect authentication",
        ],
        0,
    );

    // Record 1 is now superseded; records 2 and 3 now conflict under 'authentication'
    let text2 = stdout(s.run(&["memory", "audit"], 0));
    assert!(text2.contains("Conflicting keys: 1"));
    assert!(text2.contains("Superseded: 1"));

    // Invalidate record 2
    s.run(
        &[
            "memory",
            "invalidate",
            "2",
            "--reason",
            "Obsoleted by OpenID Connect migration",
        ],
        0,
    );

    let text3 = stdout(s.run(&["memory", "audit"], 0));
    assert!(text3.contains("Conflicting keys: 0"));
    assert!(text3.contains("Active records: 1"));
    assert!(text3.contains("Superseded: 1, Invalidated: 1"));
    assert!(text3.contains("Native context readiness: READY"));
}

#[test]
fn audit_invalidation_and_archive_lifecycle() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &["memory", "add", "--kind", "requirement", "Require TLS 1.2"],
        0,
    );
    s.run(
        &[
            "memory",
            "invalidate",
            "1",
            "--reason",
            "Upgraded to TLS 1.3 only",
        ],
        0,
    );

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Active records: 0"));
    assert!(text.contains("Valid requirements: 0"));
    assert!(text.contains("Invalidated: 1"));
    assert!(text.contains("Native context readiness: READY"));
}

#[test]
fn audit_unknown_provenance_diagnostics() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "Keep connections pooled",
        ],
        0,
    );

    // Simulate legacy migration provenance: source='legacy-unknown', actor='unknown'
    s.db()
        .execute(
            "UPDATE memory_origins SET source = 'legacy-unknown', actor = 'unknown' WHERE memory_id = 1",
            [],
        )
        .unwrap();

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Unknown provenance: 1"));
    assert!(text.contains("(requirements: 1)"));
    assert!(text.contains("Native context readiness: NEEDS ATTENTION"));
    assert!(text.contains("Active requirement #1 has unknown provenance (legacy-unknown/unknown)"));

    // Verify swapctx context omits it and fails mandatory completeness
    let ctx = stdout(s.run(&["context"], 0));
    let db = s.db();
    let complete: bool = db
        .query_row(
            "SELECT json_extract(?1, '$.coverage.mandatory_context_complete')",
            [&ctx],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!complete);
}

#[test]
fn audit_sensitive_marker_detection_without_content_leak() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let secret_payload = "api_key = AKIA1234567890123456 in production";
    s.run(&["memory", "add", "--allow-sensitive", secret_payload], 0);

    let text = stdout(s.run(&["memory", "audit"], 0));
    // The secret text itself must never be displayed
    assert!(!text.contains("AKIA1234567890123456"));
    assert!(text.contains("sensitive secret markers detected"));
    assert!(text.contains("Native context readiness: NEEDS ATTENTION"));
}

#[test]
fn audit_does_not_mutate_existing_data() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--key",
            "audit_key",
            "Stable memory entry",
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
            "System config",
        ],
        0,
    );

    // Snapshot counts and row hashes before audit
    let mem_count_before: i64 = s
        .db()
        .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    let meta_count_before: i64 = s
        .db()
        .query_row("SELECT count(*) FROM memory_metadata", [], |r| r.get(0))
        .unwrap();
    let origins_count_before: i64 = s
        .db()
        .query_row("SELECT count(*) FROM memory_origins", [], |r| r.get(0))
        .unwrap();
    let events_count_before: i64 = s
        .db()
        .query_row("SELECT count(*) FROM memory_events", [], |r| r.get(0))
        .unwrap();
    let rules_count_before: i64 = s
        .db()
        .query_row("SELECT count(*) FROM rules", [], |r| r.get(0))
        .unwrap();

    // Execute multiple audits (both text and json modes)
    s.run(&["memory", "audit"], 0);
    s.run(&["memory", "audit", "--json"], 0);
    s.run(&["memory", "audit"], 0);

    // Assert zero mutations
    let db = s.db();
    let mem_count_after: i64 = db
        .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    let meta_count_after: i64 = db
        .query_row("SELECT count(*) FROM memory_metadata", [], |r| r.get(0))
        .unwrap();
    let origins_count_after: i64 = db
        .query_row("SELECT count(*) FROM memory_origins", [], |r| r.get(0))
        .unwrap();
    let events_count_after: i64 = db
        .query_row("SELECT count(*) FROM memory_events", [], |r| r.get(0))
        .unwrap();
    let rules_count_after: i64 = db
        .query_row("SELECT count(*) FROM rules", [], |r| r.get(0))
        .unwrap();

    assert_eq!(mem_count_before, mem_count_after);
    assert_eq!(meta_count_before, meta_count_after);
    assert_eq!(origins_count_before, origins_count_after);
    assert_eq!(events_count_before, events_count_after);
    assert_eq!(rules_count_before, rules_count_after);
}

#[test]
fn audit_large_store_execution() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Insert 120 memories in batches
    {
        let mut db = s.db();
        let tx = db.transaction().unwrap();
        for i in 1..=120 {
            tx.execute(
                "INSERT INTO memories(project_id, kind, content) VALUES (1, 'note', ?1)",
                [&format!("Batch item {i} content")],
            )
            .unwrap();
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO memory_metadata(memory_id, state) VALUES (?1, 'active')",
                [id],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO memory_origins(memory_id, origin_id, source, actor, recorded_at) VALUES (?1, lower(hex(randomblob(16))), 'script', 'tester', 100000)",
                [id],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO memory_events(memory_id, action, detail, recorded_at) VALUES (?1, 'add', 'bulk', 100000)",
                [id],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Total records: 120"));
    assert!(text.contains("Active records: 120"));
    assert!(text.contains("Native context readiness: READY"));
}

#[test]
fn audit_insufficient_context_coverage_budget() {
    let s = Sandbox::new();
    s.run(&["init"], 0);

    // Add a requirement whose content is 7500 bytes (larger than 7168 PACKET_BYTES)
    let huge_content = "R".repeat(7500);
    s.run(
        &["memory", "add", "--kind", "requirement", &huge_content],
        0,
    );

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Native context readiness: NEEDS ATTENTION"));
    assert!(text.contains("Mandatory requirements: 0/1 delivered"));

    // Verify consistency with swapctx context
    let ctx = stdout(s.run(&["context", "--max-bytes", "7168"], 0));
    let db = s.db();
    let complete: bool = db
        .query_row(
            "SELECT json_extract(?1, '$.coverage.mandatory_context_complete')",
            [&ctx],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!complete);
}

#[test]
fn audit_reports_legacy_schema_version_gracefully() {
    let s = Sandbox::new();
    fs::create_dir(&s.data).unwrap();
    let conn = s.db();
    conn.execute_batch(include_str!("fixtures/v0.sql")).unwrap();
    conn.execute_batch(include_str!("../src/schema_v2.sql"))
        .unwrap();
    conn.pragma_update(None, "application_id", 0x53574358)
        .unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
    conn.execute(
        "INSERT INTO projects(id, path) VALUES (1, ?1)",
        rusqlite::params![s.project.canonicalize().unwrap().to_str().unwrap()],
    )
    .unwrap();
    drop(conn);

    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(text.contains("Native context readiness: BLOCKED"));
    assert!(text.contains("Upgrade database schema to version 3"));
}

#[test]
fn audit_json_escapes_control_characters_in_diagnostics() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["memory", "add", "--key", "multi\nline\tkey", "first"], 0);
    s.run(&["memory", "add", "--key", "multi\nline\tkey", "second"], 0);
    let json = stdout(s.run(&["memory", "audit", "--json"], 0));
    assert!(s
        .db()
        .query_row("SELECT json_valid(?1)", [&json], |r| r.get::<_, bool>(0))
        .unwrap());
}

#[test]
fn audit_sensitive_optional_records_require_review_and_action() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--allow-sensitive",
            "api_key = AKIA1234567890123456",
        ],
        0,
    );
    let json = stdout(s.run(&["memory", "audit", "--json"], 0));
    let (reviews, actions): (i64, String) = s.db().query_row(
        "SELECT json_extract(?1, '$.quality.review_recommended'), json_extract(?1, '$.suggested_actions')",
        [&json], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(reviews, 1);
    assert!(!actions.contains("No actions needed"));
    assert!(!json.contains("AKIA1234567890123456"));
}

#[test]
fn audit_missing_origin_reports_blocked_without_mutation() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["memory", "add", "ordinary note"], 0);
    s.db().execute("DELETE FROM memory_origins", []).unwrap();
    let before = fs::read(s.data.join("swapctx.db")).unwrap();
    let json = stdout(s.run(&["memory", "audit", "--json"], 0));
    let (readiness, unknown): (String, i64) = s.db().query_row(
        "SELECT json_extract(?1, '$.readiness'), json_extract(?1, '$.quality.unknown_provenance')",
        [&json], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(readiness, "BLOCKED");
    assert_eq!(unknown, 1);
    assert_eq!(before, fs::read(s.data.join("swapctx.db")).unwrap());
}

#[test]
fn audit_invalid_rule_diagnostics_are_valid_json_and_terminal_safe() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.db()
        .execute(
            "INSERT INTO rules(project_id, pattern, reason) VALUES(1, ?1, 'test')",
            ["[\n\t\u{1b}bad"],
        )
        .unwrap();
    let json = stdout(s.run(&["memory", "audit", "--json"], 0));
    assert!(s
        .db()
        .query_row("SELECT json_valid(?1)", [&json], |r| r.get::<_, bool>(0))
        .unwrap());
    let text = stdout(s.run(&["memory", "audit"], 0));
    assert!(!text.contains('\u{1b}'));
    assert!(text.contains("BLOCKED"));
}

#[test]
fn audit_exact_duplicates_are_scoped_reviewable_and_never_auto_merged() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    for kind in ["note", "NOTE", "decision"] {
        s.run(
            &[
                "memory",
                "add",
                "--kind",
                kind,
                "Preserve startup observations",
            ],
            0,
        );
    }
    for key in ["first", "second"] {
        s.run(
            &[
                "memory",
                "add",
                "--key",
                key,
                "Preserve startup observations",
            ],
            0,
        );
    }
    let before = fs::read(s.data.join("swapctx.db")).unwrap();
    let json = stdout(s.run(&["memory", "audit", "--json"], 0));
    let (groups, records, ids, reviews): (i64, i64, String, i64) = s.db().query_row(
        "SELECT json_extract(?1,'$.quality.duplicate_groups'), json_extract(?1,'$.quality.duplicate_records'), json_extract(?1,'$.quality.duplicates'), json_extract(?1,'$.quality.review_recommended')",
        [&json], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    assert_eq!((groups, records, reviews), (1, 2, 2));
    assert_eq!(ids, r#"[["1","2"]]"#);
    assert!(json.contains("NEEDS ATTENTION"));
    assert!(!json.contains("Preserve startup observations"));
    assert_eq!(before, fs::read(s.data.join("swapctx.db")).unwrap());
    let child = s.project.join("child");
    fs::create_dir(&child).unwrap();
    assert!(s
        .command(&child, &["init"])
        .output()
        .unwrap()
        .status
        .success());
    let child_output = s
        .command(&child, &["memory", "audit", "--json"])
        .output()
        .unwrap();
    assert!(child_output.status.success());
    let child_json = stdout(child_output);
    assert!(child_json.contains("\"duplicate_groups\":0"));
    s.run(&["memory", "archive", "2"], 0);
    let resolved = stdout(s.run(&["memory", "audit", "--json"], 0));
    assert!(resolved.contains("\"duplicate_groups\":0"));
    assert!(resolved.contains("\"readiness\":\"READY\""));
}

#[test]
fn audit_duplicate_diagnostics_do_not_expose_sensitive_content() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    for _ in 0..2 {
        s.run(
            &[
                "memory",
                "add",
                "--allow-sensitive",
                "api_key = AKIA1234567890123456",
            ],
            0,
        );
    }
    let json = stdout(s.run(&["memory", "audit", "--json"], 0));
    assert!(json.contains("\"duplicate_groups\":1"));
    assert!(json.contains("\"review_recommended\":2"));
    assert!(!json.contains("AKIA1234567890123456"));
}

#[test]
fn audit_duplicate_ids_preserve_64_bit_identity() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    for _ in 0..2 {
        s.run(&["memory", "add", "same observation"], 0);
    }
    let db = s.db();
    db.execute_batch(
        "BEGIN;
        PRAGMA defer_foreign_keys=ON;
        UPDATE memories SET id=id+9007199254740992;
        UPDATE memory_metadata SET memory_id=memory_id+9007199254740992;
        UPDATE memory_origins SET memory_id=memory_id+9007199254740992;
        UPDATE memory_events SET memory_id=memory_id+9007199254740992;
        COMMIT;",
    )
    .unwrap();
    drop(db);
    let json = stdout(s.run(&["memory", "audit", "--json"], 0));
    let ids: String = s
        .db()
        .query_row(
            "SELECT json_extract(?1,'$.quality.duplicates')",
            [&json],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ids, r#"[["9007199254740993","9007199254740994"]]"#);
}
