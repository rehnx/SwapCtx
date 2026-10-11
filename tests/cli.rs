//! Exercise the real executable with isolated storage, never the user's database.
use rusqlite::{params, Connection};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

struct Sandbox {
    temp: TempDir,
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
            temp,
            project,
            data,
        }
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_swapctx"));
        cmd.current_dir(cwd)
            .arg("--data-dir")
            .arg(&self.data)
            .args(args);
        cmd
    }

    fn run_at(&self, cwd: &Path, args: &[&str], code: i32) -> Output {
        let output = self.command(cwd, args).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(code),
            "{args:?}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn run(&self, args: &[&str], code: i32) -> Output {
        self.run_at(&self.project, args, code)
    }

    fn db(&self) -> Connection {
        Connection::open(self.data.join("swapctx.db")).unwrap()
    }
}

fn stdout(output: Output) -> String {
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn reads_and_privacy_do_not_initialize_storage() {
    let s = Sandbox::new();
    s.run(&["privacy"], 0);
    s.run(&["status"], 1);
    s.run(&["memory", "recent"], 1);
    assert!(!s.data.exists());
}

#[test]
fn persists_unicode_and_literal_search_across_processes() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "Preserve 世界 100%_done\\path",
        ],
        0,
    );
    s.run(&["memory", "add", "other record"], 0);
    let text = stdout(s.run(&["memory", "search", "%_done\\path"], 0));
    assert!(text.contains("世界"));
    assert!(!text.contains("other record"));
    assert!(stdout(s.run(&["memory", "search", "REQUIREMENT"], 0)).contains("Preserve"));
    s.run(&["init"], 0);
    assert!(stdout(s.run(&["status"], 0)).contains("Memory records: 2"));
    let text = stdout(s.run(&["memory", "recent", "--limit", "1"], 0));
    assert!(text.contains("other record"));
    assert!(!text.contains("Preserve"));
}

#[test]
fn separates_projects_and_resolves_subdirectories() {
    let s = Sandbox::new();
    let nested = s.project.join("nested");
    fs::create_dir(&nested).unwrap();
    s.run(&["init"], 0);
    s.run_at(&nested, &["memory", "add", "outer"], 0);
    s.run_at(&nested, &["init"], 0);
    s.run_at(&nested, &["memory", "add", "inner"], 0);
    let text = stdout(s.run_at(&nested, &["memory", "recent"], 0));
    assert!(text.contains("inner"));
    assert!(!text.contains("outer"));
    assert!(stdout(s.run(&["memory", "recent"], 0)).contains("outer"));
    s.run_at(s.temp.path(), &["status"], 1);
}

#[test]
fn protection_exit_codes_patterns_and_scope() {
    let s = Sandbox::new();
    let sub = s.project.join("src");
    fs::create_dir(&sub).unwrap();
    s.run(&["init"], 0);
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "src/auth/**",
            "--reason",
            "review",
        ],
        0,
    );
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "src/auth/**",
            "--reason",
            "review",
        ],
        1,
    );
    s.run(&["protect", "check", "src/auth/login.rs"], 2);
    s.run(&["protect", "check", "src/auth"], 2);
    s.run_at(&sub, &["protect", "check", "auth/new.rs"], 2);
    let absolute = s.project.join("src/auth/new.rs");
    s.run(&["protect", "check", absolute.to_str().unwrap()], 2);
    s.run(&["protect", "check", "src/ui/new.rs"], 0);
    s.run(&["protect", "check", "../outside"], 1);
    let other = s.temp.path().join("other");
    fs::create_dir(&other).unwrap();
    s.run_at(&other, &["init"], 0);
    s.run_at(&other, &["protect", "remove", "1"], 1);
    s.run(&["protect", "remove", "1"], 0);
    s.run(&["protect", "check", "src/auth/new.rs"], 0);
}

#[test]
fn input_errors_do_not_write_records_and_controls_are_escaped() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["memory", "add", " "], 1);
    s.run(&["memory", "search", " "], 1);
    s.run(&["memory", "recent", "--limit", "0"], 2);
    s.run(&["memory", "recent", "--limit", "101"], 2);
    for path in ["src\\auth\\**", "../x", "C:foo", "NUL", "src/name."] {
        s.run(&["protect", "add", "--path", path, "--reason", "review"], 1);
    }
    assert!(stdout(s.run(&["status"], 0)).contains("Memory records: 0"));
    s.run(&["memory", "add", "a\x1b[2J\nb"], 0);
    let text = stdout(s.run(&["memory", "recent"], 0));
    assert!(!text.contains('\x1b'));
    assert!(text.contains("\\u{1b}"));
    assert!(text.contains("\\nb"));
    assert_eq!(
        s.db()
            .query_row("SELECT content FROM memories", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "a\x1b[2J\nb"
    );
}

#[test]
fn migrates_original_schema_preserving_ids_and_content() {
    let s = Sandbox::new();
    fs::create_dir(&s.data).unwrap();
    {
        let conn = s.db();
        conn.execute_batch(include_str!("fixtures/v0.sql")).unwrap();
        conn.execute(
            "INSERT INTO projects(id,path) VALUES (7,?1)",
            params![s.project.canonicalize().unwrap().to_str().unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memories(id,project_id,kind,content) VALUES (42,7,'requirement','keep')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rules(id,project_id,pattern,reason) VALUES (9,7,'src/**','review')",
            [],
        )
        .unwrap();
    }
    // Legacy reads work without silently migrating the database.
    assert!(stdout(s.run(&["memory", "recent"], 0)).contains("#42"));
    assert_eq!(
        s.db()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    s.run(&["init"], 0);
    s.run(&["init"], 0);
    assert!(stdout(s.run(&["memory", "recent"], 0)).contains("#42"));
    assert!(stdout(s.run(&["protect", "list"], 0)).contains("#9"));
    assert_eq!(
        s.db()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        s.db()
            .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}

#[test]
fn rejects_future_foreign_corrupt_and_invalid_stored_rules() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    {
        let conn = s.db();
        conn.pragma_update(None, "user_version", 99).unwrap();
    }
    s.run(&["init"], 1);
    s.run(&["status"], 1);
    assert_eq!(
        s.db()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
    {
        let conn = s.db();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn.execute(
            "INSERT INTO rules(project_id,pattern,reason) VALUES (1,'../bad','bad')",
            [],
        )
        .unwrap();
    }
    s.run(&["protect", "check", "src/file"], 1);
    let foreign = Sandbox::new();
    fs::create_dir(&foreign.data).unwrap();
    foreign
        .db()
        .execute("CREATE TABLE unrelated(data TEXT)", [])
        .unwrap();
    foreign.run(&["init"], 1);
    assert_eq!(
        foreign
            .db()
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type='table'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    let corrupt = Sandbox::new();
    fs::create_dir(&corrupt.data).unwrap();
    fs::write(corrupt.data.join("swapctx.db"), b"not a database").unwrap();
    corrupt.run(&["init"], 1);
    assert_eq!(
        fs::read(corrupt.data.join("swapctx.db")).unwrap(),
        b"not a database"
    );
}

#[test]
fn concurrent_writers_preserve_every_record() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let mut children = Vec::new();
    for i in 0..8 {
        children.push(
            s.command(&s.project, &["memory", "add", &format!("record {i}")])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert!(stdout(s.run(&["status"], 0)).contains("Memory records: 8"));
}

#[test]
fn boundary_policy_precedence_is_persistent_and_explicit() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "protect", "add", "--path", "src/**", "--reason", "observe", "--policy", "audit",
        ],
        0,
    );
    assert!(
        String::from_utf8(s.run(&["protect", "check", "src/file"], 0).stderr)
            .unwrap()
            .contains("AUDIT")
    );
    s.run(
        &[
            "protect", "add", "--path", "src/**", "--reason", "review", "--policy", "ask",
        ],
        0,
    );
    let ask = s.run(&["protect", "check", "src/file"], 3);
    assert!(String::from_utf8(ask.stderr)
        .unwrap()
        .contains("has not requested or granted approval"));
    s.run(
        &["protect", "add", "--path", "src/**", "--reason", "deny"],
        0,
    );
    s.run(&["protect", "check", "src/file"], 2);
    let list = stdout(s.run(&["protect", "list"], 0));
    for label in ["policy=deny", "policy=ask", "policy=audit"] {
        assert!(list.contains(label));
    }
    s.run(&["protect", "remove", "3"], 0);
    s.run(&["protect", "check", "src/file"], 3);
    s.run(
        &[
            "protect", "add", "--path", "src/**", "--reason", "review", "--policy", "ask",
        ],
        1,
    );
}

#[test]
fn lifecycle_preserves_history_and_labels_explicit_conflicts() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--key",
            "auth",
            "original requirement",
        ],
        0,
    );
    s.run(
        &["memory", "add", "--key", "auth", "competing requirement"],
        0,
    );
    assert!(stdout(s.run(&["memory", "recent"], 0)).contains("CONFLICT: 2 active records"));
    s.run(&["memory", "supersede", "1", "replacement requirement"], 0);
    assert!(stdout(s.run(&["memory", "get", "1"], 0)).contains("superseded_by=Some(3)"));
    let active = stdout(s.run(&["memory", "recent"], 0));
    assert!(!active.contains("original requirement"));
    assert!(active.contains("replacement requirement"));
    s.run(&["memory", "archive", "2"], 0);
    let active = stdout(s.run(&["memory", "search", "requirement"], 0));
    assert!(!active.contains("CONFLICT"));
    assert!(!active.contains("competing requirement"));
    let history = stdout(s.run(&["memory", "recent", "--include-inactive"], 0));
    for content in [
        "original requirement",
        "competing requirement",
        "replacement requirement",
    ] {
        assert!(history.contains(content));
    }
    s.run(&["memory", "supersede", "1", "must not be inserted"], 1);
    s.run(&["memory", "archive", "1"], 1);
    assert_eq!(
        s.db()
            .query_row("SELECT count(*) FROM memories", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
    s.run(&["doctor"], 0);
}

#[test]
fn freshness_is_explicit_and_stale_requirements_remain_visible() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "memory",
            "add",
            "--kind",
            "requirement",
            "--expires-in-days",
            "1",
            "keep this requirement",
        ],
        0,
    );
    s.db()
        .execute(
            "UPDATE memory_metadata SET expires_at=1 WHERE memory_id=1",
            [],
        )
        .unwrap();
    assert!(stdout(s.run(&["memory", "recent"], 0)).contains("STALE"));
    assert!(
        !stdout(s.run(&["memory", "recent", "--fresh-only"], 0)).contains("keep this requirement")
    );
    s.run(&["memory", "review", "1"], 0);
    assert!(stdout(s.run(&["memory", "get", "1"], 0)).contains("STALE"));
    s.run(&["memory", "review", "1", "--expires-in-days", "2"], 0);
    assert!(!stdout(s.run(&["memory", "get", "1"], 0)).contains("STALE"));
    s.run(&["memory", "review", "1", "--clear-expiry"], 0);
    assert!(stdout(s.run(&["memory", "get", "1"], 0)).contains("expires_unix=None"));
}

#[test]
fn lifecycle_operations_cannot_read_or_modify_another_project() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["memory", "add", "private project record"], 0);
    let other = s.temp.path().join("other");
    fs::create_dir(&other).unwrap();
    s.run_at(&other, &["init"], 0);
    for args in [
        &["memory", "get", "1"][..],
        &["memory", "archive", "1"],
        &["memory", "review", "1"],
        &["memory", "supersede", "1", "bad replacement"],
    ] {
        let out = s.run_at(&other, args, 1);
        assert!(!String::from_utf8(out.stdout)
            .unwrap()
            .contains("private project record"));
    }
    assert_eq!(
        s.db()
            .query_row("SELECT count(*) FROM memories", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(stdout(s.run(&["memory", "recent"], 0)).contains("private project record"));
}

#[test]
fn potential_secrets_require_explicit_storage_and_reveal() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let value = "PASSWORD=example-sensitive-value";
    let out = s.run(&["memory", "add", value], 1);
    assert!(!String::from_utf8(out.stderr).unwrap().contains(value));
    s.run(
        &["protect", "add", "--path", "src/**", "--reason", value],
        1,
    );
    assert!(stdout(s.run(&["status"], 0)).contains("Memory records: 0"));
    s.run(&["memory", "add", "--allow-sensitive", value], 0);
    assert!(!stdout(s.run(&["memory", "recent"], 0)).contains(value));
    assert!(!stdout(s.run(&["memory", "search", "example-sensitive"], 0)).contains(value));
    assert!(!stdout(s.run(&["memory", "get", "1"], 0)).contains(value));
    assert!(stdout(s.run(&["memory", "get", "1", "--reveal"], 0)).contains(value));
    assert_eq!(
        s.db()
            .query_row("SELECT content FROM memories", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        value
    );
}

#[test]
fn version_one_reads_remain_read_only_and_writes_upgrade_without_loss() {
    let s = Sandbox::new();
    fs::create_dir(&s.data).unwrap();
    {
        let conn = s.db();
        conn.execute_batch(include_str!("fixtures/v0.sql")).unwrap();
        conn.pragma_update(None, "application_id", 0x53574358)
            .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute(
            "INSERT INTO projects(id,path) VALUES(7,?1)",
            [s.project.canonicalize().unwrap().to_str().unwrap()],
        )
        .unwrap();
        conn.execute("INSERT INTO memories(id,project_id,kind,content,created_at) VALUES(42,7,'requirement','legacy text','2020-01-01 00:00:00')",[]).unwrap();
        conn.execute(
            "INSERT INTO rules(id,project_id,pattern,reason) VALUES(9,7,'src/**','legacy rule')",
            [],
        )
        .unwrap();
    }
    assert!(stdout(s.run(&["memory", "get", "42"], 0)).contains("2020-01-01"));
    s.run(&["protect", "check", "src/file"], 2);
    s.run(&["doctor"], 0);
    assert_eq!(
        s.db()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    s.run(&["memory", "add", "new record"], 0);
    assert_eq!(
        s.db()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert!(stdout(s.run(&["memory", "get", "42"], 0)).contains("legacy text"));
    assert!(stdout(s.run(&["protect", "list"], 0)).contains("#9"));
}

#[test]
fn diagnostics_detect_bad_relations_without_exposing_content() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["memory", "add", "old text"], 0);
    let other = s.temp.path().join("other");
    fs::create_dir(&other).unwrap();
    s.run_at(&other, &["init"], 0);
    s.run_at(&other, &["memory", "add", "other text"], 0);
    s.db()
        .execute(
            "UPDATE memory_metadata SET state='superseded',superseded_by=2 WHERE memory_id=1",
            [],
        )
        .unwrap();
    s.run(&["doctor"], 1);
    let target = s.temp.path().join("bad-backup.db");
    s.run(&["backup", target.to_str().unwrap()], 1);
    assert!(!target.exists());
}

#[test]
fn snapshot_can_be_reopened_as_a_complete_store_and_never_overwrites() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["memory", "add", "original"], 0);
    s.run(&["memory", "supersede", "1", "replacement"], 0);
    s.run(
        &[
            "protect", "add", "--path", "src/**", "--reason", "review", "--policy", "ask",
        ],
        0,
    );
    let target = s.temp.path().join("snapshot.db");
    s.run(&["backup", target.to_str().unwrap()], 0);
    let before = fs::read(&target).unwrap();
    s.run(&["backup", target.to_str().unwrap()], 1);
    assert_eq!(fs::read(&target).unwrap(), before);
    let restored = s.temp.path().join("restored");
    fs::create_dir(&restored).unwrap();
    fs::copy(&target, restored.join("swapctx.db")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_swapctx"))
        .current_dir(&s.project)
        .arg("--data-dir")
        .arg(&restored)
        .args(["memory", "recent", "--include-inactive"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("original"));
    assert!(text.contains("replacement"));
    assert!(text.contains("superseded_by=Some(2)"));
    let restored_conn = Connection::open(restored.join("swapctx.db")).unwrap();
    assert_eq!(
        restored_conn
            .query_row("SELECT policy FROM rule_policies", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "ask"
    );
    s.run(&["memory", "add", "after snapshot"], 0);
    assert_eq!(
        restored_conn
            .query_row("SELECT count(*) FROM memories", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn online_snapshot_reads_wal_and_excludes_uncommitted_changes() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["memory", "add", "committed record"], 0);
    let writer = s.db();
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer
        .execute(
            "INSERT INTO memories(project_id,kind,content) VALUES(1,'note','committed WAL record')",
            [],
        )
        .unwrap();
    writer.execute_batch("BEGIN IMMEDIATE; INSERT INTO memories(project_id,kind,content) VALUES(1,'note','uncommitted record');").unwrap();
    let path = s.temp.path().join("wal-snapshot.db");
    s.run(&["backup", path.to_str().unwrap()], 0);
    let copy = Connection::open(&path).unwrap();
    assert_eq!(
        copy.query_row("SELECT count(*) FROM memories", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        copy.query_row(
            "SELECT count(*) FROM memories WHERE content='uncommitted record'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        copy.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    writer.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn concurrent_supersessions_have_one_winner_and_no_partial_replacements() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["memory", "add", "original"], 0);
    let mut children = Vec::new();
    for i in 0..4 {
        children.push(
            s.command(
                &s.project,
                &["memory", "supersede", "1", &format!("replacement {i}")],
            )
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
        );
    }
    let mut wins = 0;
    for child in children {
        let out = child.wait_with_output().unwrap();
        if out.status.success() {
            wins += 1;
        } else {
            assert_eq!(out.status.code(), Some(1));
        }
    }
    assert_eq!(wins, 1);
    assert_eq!(
        s.db()
            .query_row("SELECT count(*) FROM memories", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    s.run(&["doctor"], 0);
}

#[test]
fn lock_timeout_does_not_lose_data_and_retry_can_succeed() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let lock = s.db();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    s.run(&["memory", "add", "must not be saved"], 1);
    lock.execute_batch("ROLLBACK").unwrap();
    s.run(&["memory", "add", "retry"], 0);
    assert_eq!(
        s.db()
            .query_row("SELECT count(*) FROM memories", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
#[ignore = "Run explicitly in release mode to record timings, not a CI timing threshold"]
fn memory_performance_on_large_store() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let mut conn = s.db();
    let tx = conn.transaction().unwrap();
    {
        let mut add = tx
            .prepare("INSERT INTO memories(project_id,kind,content) VALUES(1,'note',?1)")
            .unwrap();
        for i in 0..100_000 {
            add.execute([format!("record {i} {}", "context ".repeat(32))])
                .unwrap();
        }
    }
    tx.commit().unwrap();
    drop(conn);
    for args in [
        &["memory", "recent"][..],
        &["memory", "search", "absent-literal"],
        &["memory", "get", "1"],
    ] {
        let mut timings = Vec::new();
        for _ in 0..20 {
            let start = std::time::Instant::now();
            s.run(args, 0);
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        timings.sort_by(f64::total_cmp);
        println!(
            "100000 records {args:?}: p50={:.2}ms p95={:.2}ms (20 complete CLI processes)",
            timings[10], timings[18]
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_junction_aliases_and_external_destinations() {
    fn junction(target: &Path, link: &Path) {
        let out = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "Junction setup failed: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let s = Sandbox::new();
    fs::create_dir(s.project.join("real")).unwrap();
    let outside = s.temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    junction(&s.project.join("real"), &s.project.join("alias"));
    junction(&outside, &s.project.join("external"));
    junction(&s.project, &s.project.join("loop"));
    s.run(&["init"], 0);
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "alias/**",
            "--reason",
            "alias protected",
        ],
        0,
    );
    s.run(&["protect", "check", "alias/missing/file"], 2);
    s.run(&["protect", "remove", "1"], 0);
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "real/**",
            "--reason",
            "destination protected",
        ],
        0,
    );
    s.run(&["protect", "check", "alias/missing/file"], 2);
    s.run(&["protect", "check", "external/missing/file"], 1);
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "loop/**",
            "--reason",
            "root alias",
        ],
        0,
    );
    s.run(&["protect", "check", "loop/new-file"], 2);
    fs::write(outside.join("keep"), "untouched").unwrap();
    // Remove the junctions themselves before TempDir cleanup.
    fs::remove_dir(s.project.join("alias")).unwrap();
    fs::remove_dir(s.project.join("external")).unwrap();
    fs::remove_dir(s.project.join("loop")).unwrap();
    assert_eq!(
        fs::read_to_string(outside.join("keep")).unwrap(),
        "untouched"
    );
}

#[cfg(windows)]
#[test]
fn windows_case_and_ambiguous_paths() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(
        &[
            "protect",
            "add",
            "--path",
            "src/auth/**",
            "--reason",
            "review",
        ],
        0,
    );
    s.run(&["protect", "check", "SRC/AUTH/new.rs"], 2);
    for path in [
        "C:relative",
        "\\rooted",
        "src/file:stream",
        "src/name.",
        "src/name ",
        "src/ name",
        "src/*.rs",
        "COM¹",
    ] {
        s.run(&["protect", "check", path], 1);
    }
}

#[cfg(windows)]
#[test]
fn rejects_ambiguous_legacy_project_case_aliases() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let conn = s.db();
    let original: String = conn
        .query_row("SELECT path FROM projects", [], |r| r.get(0))
        .unwrap();
    let alias = original.to_ascii_uppercase();
    assert_ne!(alias, original);
    conn.execute("INSERT INTO projects(path) VALUES (?1)", [alias])
        .unwrap();
    drop(conn);
    s.run(&["status"], 1);
    s.run(&["init"], 1);
    assert_eq!(
        s.db()
            .query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}
