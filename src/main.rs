//! Local-only external context records and explicit path-rule checks.
mod display;
mod paths;
mod storage;

use std::{env, path::PathBuf, process::ExitCode};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::params;
use display::safe;

#[derive(Parser, Debug)]
#[command(name = "swapctx", version, about = "Local-first context memory and explicit protection-rule checks")]
struct Cli {
    /// Use an explicit local data directory, including for isolated tests.
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Register the current directory as a SwapCtx project.
    Init,
    /// Show local storage and the current project's records.
    Status,
    /// Create, list, or check project path-protection rules.
    Protect {
        #[command(subcommand)]
        action: ProtectAction,
    },
    /// Save and find project context in local SQLite.
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Explain what data stays local and where it is stored.
    Privacy,
}

#[derive(Subcommand, Debug)]
enum ProtectAction {
    /// Register a protected relative file path/glob (example: src/auth/**).
    Add {
        #[arg(long)]
        path: String,
        #[arg(long)]
        reason: String,
    },
    /// Show the current project's rules.
    List,
    /// Explicit snapshot check only (exit code 2 = protected).
    Check { path: String },
    /// Delete one rule by its numeric ID.
    Remove { id: i64 },
}

#[derive(Subcommand, Debug)]
enum MemoryAction {
    /// Save a deliberate project decision, requirement, or note.
    Add {
        #[arg(long, default_value = "note")]
        kind: String,
        text: String,
    },
    /// Literal substring search; ASCII case insensitive, no semantic retrieval.
    Search {
        query: String,
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
    },
    /// List the latest stored project context records.
    Recent {
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
    },
}

fn storage_path(data_dir: Option<PathBuf>) -> Result<PathBuf> {
    let directory = match data_dir {
        Some(path) => {
            if path.as_os_str().is_empty() { bail!("Data directory must not be empty"); }
            if path.has_root() != path.is_absolute()
                || (!path.is_absolute() && path.components().any(|p| matches!(p, std::path::Component::Prefix(_))))
            {
                bail!("Data directory must be an ordinary relative path or an absolute path");
            }
            if path.is_absolute() { path } else { env::current_dir()?.join(path) }
        }
        None => {
            #[cfg(windows)]
            let base = env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty())
                .map(PathBuf::from).context("LOCALAPPDATA is missing; supply --data-dir")?;
            #[cfg(not(windows))]
            let base = env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty())
                .map(PathBuf::from).or_else(|| env::var_os("HOME").filter(|v| !v.is_empty())
                    .map(|h| PathBuf::from(h).join(".local/share")))
                .context("Cannot locate local data directory; supply --data-dir")?;
            if !base.is_absolute() { bail!("Local data directory must be absolute; supply --data-dir"); }
            base.join("SwapCtx")
        }
    };
    Ok(directory.join("swapctx.db"))
}

fn validate_text(label: &str, value: &str, max_bytes: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > max_bytes || value.contains('\0') {
        bail!("{label} must be nonempty, contain no NUL, and use at most {max_bytes} UTF-8 bytes");
    }
    Ok(())
}

fn escape_like(term: &str) -> String {
    term.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn run(cli: Cli) -> Result<u8> {
    let db_path = storage_path(cli.data_dir)?;
    if matches!(cli.command, Command::Privacy) {
        println!("SwapCtx local privacy mode:");
        println!("All explicit memory records and rules stay in a local SQLite file.");
        println!("No AI calls, telemetry, automatic scanning, or agent integration.");
        println!("Data is NOT encrypted at rest. Local account permissions apply.");
        println!("External coding agents retain their own network behavior.");
        println!("Storage: {}", safe(&db_path.display().to_string()));
        println!("Building from source may download Rust dependencies.");
        return Ok(0);
    }
    let create = matches!(cli.command, Command::Init);
    let writable = create || matches!(cli.command,
        Command::Protect { action: ProtectAction::Add { .. } | ProtectAction::Remove { .. } }
        | Command::Memory { action: MemoryAction::Add { .. } });
    let mut conn = storage::open(&db_path, create, writable)?;
    let cwd = env::current_dir()?.canonicalize().context("Cannot resolve current directory")?;
    if create {
        storage::register(&mut conn, &cwd)?;
        println!("Initialized project: {}", safe(paths::path_text(&cwd)?));
        println!("All records stored locally: {}", safe(&db_path.display().to_string()));
        println!("No agent integration or automatic blocking is enabled.");
        return Ok(0);
    }
    let (id, root) = storage::project(&conn, &cwd)?;
    match cli.command {
        Command::Status => {
            let rules: i64 = conn.query_row("SELECT COUNT(*) FROM rules WHERE project_id = ?1", [id], |r| r.get(0))?;
            let memories: i64 = conn.query_row("SELECT COUNT(*) FROM memories WHERE project_id = ?1", [id], |r| r.get(0))?;
            println!("SWAPCTX v{} - LOCAL MODE", env!("CARGO_PKG_VERSION"));
            println!("Project: {}", safe(paths::path_text(&root)?));
            println!("Protected path rules: {rules}");
            println!("Memory records: {memories}");
            println!("Database: {}", safe(&db_path.display().to_string()));
            println!("Agent hooks: not connected");
        }
        Command::Protect { action } => match action {
            ProtectAction::Add { path, reason } => {
                paths::rule_matcher(&path)?;
                validate_text("Rule reason", &reason, 16 * 1024)?;
                let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM rules WHERE project_id=?1 AND pattern=?2 AND reason=?3)",
                    params![id, path, reason], |r| r.get(0),
                )?;
                if exists { bail!("An identical rule already exists in this project"); }
                tx.execute("INSERT INTO rules(project_id, pattern, reason) VALUES (?1, ?2, ?3)", params![id, path, reason])?;
                tx.commit()?;
                println!("Recorded rule: {}", safe(&path));
                println!("Enforcement hooks are not active.");
            }
            ProtectAction::List => {
                let mut stmt = conn.prepare("SELECT id, pattern, reason FROM rules WHERE project_id = ?1 ORDER BY id")?;
                let rows = stmt.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
                let mut count = 0;
                for row in rows {
                    let (rule_id, pattern, reason) = row?;
                    println!("#{rule_id} {} - {}", safe(&pattern), safe(&reason));
                    count += 1;
                }
                if count == 0 { println!("No protection rules yet."); }
            }
            ProtectAction::Check { path } => {
                let targets = paths::target_paths(&path, &cwd, &root)?;
                let mut stmt = conn.prepare("SELECT id, pattern, reason FROM rules WHERE project_id = ?1 ORDER BY id")?;
                let rows = stmt.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
                let mut matched = false;
                for row in rows {
                    let (rule_id, pattern, reason) = row?;
                    if paths::matches(&pattern, &targets).with_context(|| format!("Invalid stored rule #{rule_id}; check failed"))? {
                        eprintln!("PROTECTED: {} matches rule #{rule_id} '{}' - {}", safe(&targets[0]), safe(&pattern), safe(&reason));
                        matched = true;
                    }
                }
                if matched { return Ok(2); }
                println!("No protected path rule matched: {}", safe(&targets[0]));
            }
            ProtectAction::Remove { id: rule_id } => {
                if conn.execute("DELETE FROM rules WHERE id = ?1 AND project_id = ?2", params![rule_id, id])? == 0 {
                    bail!("Rule #{rule_id} not found in this project");
                }
                println!("Removed rule #{rule_id}");
            }
        },
        Command::Memory { action } => match action {
            MemoryAction::Add { kind, text } => {
                validate_text("Memory kind", &kind, 128)?;
                validate_text("Memory content", &text, 1024 * 1024)?;
                conn.execute("INSERT INTO memories(project_id, kind, content) VALUES (?1, ?2, ?3)", params![id, kind, text])?;
                println!("Saved local memory record #{}", conn.last_insert_rowid());
            }
            MemoryAction::Search { query, limit } => {
                validate_text("Search query", &query, 4096)?;
                let mut stmt = conn.prepare(
                    "SELECT id, kind, content, created_at FROM memories
                     WHERE project_id = ?1 AND (content LIKE ?2 ESCAPE '\\' OR kind LIKE ?2 ESCAPE '\\')
                     ORDER BY id DESC LIMIT ?3",
                )?;
                print_memories(&mut stmt, params![id, format!("%{}%", escape_like(&query)), limit])?;
            }
            MemoryAction::Recent { limit } => {
                let mut stmt = conn.prepare(
                    "SELECT id, kind, content, created_at FROM memories WHERE project_id = ?1 ORDER BY id DESC LIMIT ?2",
                )?;
                print_memories(&mut stmt, params![id, limit])?;
            }
        },
        Command::Init | Command::Privacy => unreachable!("handled before project lookup"),
    }
    Ok(0)
}

fn print_memories(stmt: &mut rusqlite::Statement<'_>, params: impl rusqlite::Params) -> Result<()> {
    let rows = stmt.query_map(params, |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?;
    let mut count = 0;
    for row in rows {
        let (id, kind, content, created_at) = row?;
        println!("#{id} [{}] {}\n  {}", safe(&kind), safe(&created_at), safe(&content));
        count += 1;
    }
    if count == 0 { println!("No memory records found."); }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("SwapCtx error: {}", safe(&format!("{error:#}")));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_sql_wildcards() {
        assert_eq!(escape_like("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }

    #[test]
    fn validates_bounded_text() {
        assert!(validate_text("test", "  ", 10).is_err());
        assert!(validate_text("test", "a\0b", 10).is_err());
        assert!(validate_text("test", "世界", 5).is_err());
        assert!(validate_text("test", "世界", 6).is_ok());
    }
}
