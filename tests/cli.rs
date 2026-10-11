//! Exercise the real executable with isolated storage, never the user's database.
use std::{fs, path::{Path, PathBuf}, process::{Command, Output}};
use rusqlite::{params, Connection};
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
        Self { temp, project, data }
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_swapctx"));
        cmd.current_dir(cwd).arg("--data-dir").arg(&self.data).args(args);
        cmd
    }

    fn run_at(&self, cwd: &Path, args: &[&str], code: i32) -> Output {
        let output = self.command(cwd, args).output().unwrap();
        assert_eq!(output.status.code(), Some(code), "{args:?}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        output
    }

    fn run(&self, args: &[&str], code: i32) -> Output {
        self.run_at(&self.project, args, code)
    }

    fn db(&self) -> Connection {
        Connection::open(self.data.join("swapctx.db")).unwrap()
    }
}

fn stdout(output: Output) -> String { String::from_utf8(output.stdout).unwrap() }

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
    s.run(&["memory", "add", "--kind", "requirement", "Preserve 世界 100%_done\\path"], 0);
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
    s.run(&["protect", "add", "--path", "src/auth/**", "--reason", "review"], 0);
    s.run(&["protect", "add", "--path", "src/auth/**", "--reason", "review"], 1);
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
    assert_eq!(s.db().query_row("SELECT content FROM memories", [], |r| r.get::<_, String>(0)).unwrap(), "a\x1b[2J\nb");
}

#[test]
fn migrates_original_schema_preserving_ids_and_content() {
    let s = Sandbox::new();
    fs::create_dir(&s.data).unwrap();
    {
        let conn = s.db();
        conn.execute_batch(include_str!("fixtures/v0.sql")).unwrap();
        conn.execute("INSERT INTO projects(id,path) VALUES (7,?1)", params![s.project.canonicalize().unwrap().to_str().unwrap()]).unwrap();
        conn.execute("INSERT INTO memories(id,project_id,kind,content) VALUES (42,7,'requirement','keep')", []).unwrap();
        conn.execute("INSERT INTO rules(id,project_id,pattern,reason) VALUES (9,7,'src/**','review')", []).unwrap();
    }
    // Legacy reads work without silently migrating the database.
    assert!(stdout(s.run(&["memory", "recent"], 0)).contains("#42"));
    assert_eq!(s.db().pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 0);
    s.run(&["init"], 0);
    s.run(&["init"], 0);
    assert!(stdout(s.run(&["memory", "recent"], 0)).contains("#42"));
    assert!(stdout(s.run(&["protect", "list"], 0)).contains("#9"));
    assert_eq!(s.db().pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 1);
    assert_eq!(s.db().query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)).unwrap(), "ok");
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
    assert_eq!(s.db().pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 99);
    {
        let conn = s.db();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO rules(project_id,pattern,reason) VALUES (1,'../bad','bad')", []).unwrap();
    }
    s.run(&["protect", "check", "src/file"], 1);
    let foreign = Sandbox::new();
    fs::create_dir(&foreign.data).unwrap();
    foreign.db().execute("CREATE TABLE unrelated(data TEXT)", []).unwrap();
    foreign.run(&["init"], 1);
    assert_eq!(foreign.db().query_row("SELECT count(*) FROM sqlite_schema WHERE type='table'", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    let corrupt = Sandbox::new();
    fs::create_dir(&corrupt.data).unwrap();
    fs::write(corrupt.data.join("swapctx.db"), b"not a database").unwrap();
    corrupt.run(&["init"], 1);
    assert_eq!(fs::read(corrupt.data.join("swapctx.db")).unwrap(), b"not a database");
}

#[test]
fn concurrent_writers_preserve_every_record() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let mut children = Vec::new();
    for i in 0..8 {
        children.push(s.command(&s.project, &["memory", "add", &format!("record {i}")])
            .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap());
    }
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    assert!(stdout(s.run(&["status"], 0)).contains("Memory records: 8"));
}

#[cfg(windows)]
#[test]
fn windows_junction_aliases_and_external_destinations() {
    fn junction(target: &Path, link: &Path) {
        let out = Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"])
            .arg(link).arg(target).output().unwrap();
        assert!(out.status.success(), "Junction setup failed: {} {}",
            String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
    let s = Sandbox::new();
    fs::create_dir(s.project.join("real")).unwrap();
    let outside = s.temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    junction(&s.project.join("real"), &s.project.join("alias"));
    junction(&outside, &s.project.join("external"));
    s.run(&["init"], 0);
    s.run(&["protect", "add", "--path", "alias/**", "--reason", "alias protected"], 0);
    s.run(&["protect", "check", "alias/missing/file"], 2);
    s.run(&["protect", "remove", "1"], 0);
    s.run(&["protect", "add", "--path", "real/**", "--reason", "destination protected"], 0);
    s.run(&["protect", "check", "alias/missing/file"], 2);
    s.run(&["protect", "check", "external/missing/file"], 1);
    fs::write(outside.join("keep"), "untouched").unwrap();
    // Remove the junctions themselves before TempDir cleanup.
    fs::remove_dir(s.project.join("alias")).unwrap();
    fs::remove_dir(s.project.join("external")).unwrap();
    assert_eq!(fs::read_to_string(outside.join("keep")).unwrap(), "untouched");
}

#[cfg(windows)]
#[test]
fn windows_case_and_ambiguous_paths() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    s.run(&["protect", "add", "--path", "src/auth/**", "--reason", "review"], 0);
    s.run(&["protect", "check", "SRC/AUTH/new.rs"], 2);
    for path in ["C:relative", "\\rooted", "src/file:stream", "src/name.", "src/name ", "src/ name", "src/*.rs", "COM¹"] {
        s.run(&["protect", "check", path], 1);
    }
}

#[cfg(windows)]
#[test]
fn rejects_ambiguous_legacy_project_case_aliases() {
    let s = Sandbox::new();
    s.run(&["init"], 0);
    let conn = s.db();
    let original: String = conn.query_row("SELECT path FROM projects", [], |r| r.get(0)).unwrap();
    let alias = original.to_ascii_uppercase();
    assert_ne!(alias, original);
    conn.execute("INSERT INTO projects(path) VALUES (?1)", [alias]).unwrap();
    drop(conn);
    s.run(&["status"], 1);
    s.run(&["init"], 1);
    assert_eq!(s.db().query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
}
