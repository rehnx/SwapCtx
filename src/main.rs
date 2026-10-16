//! Local-only external context records and explicit boundary-policy checks.
mod backup;
mod codex;
mod context;
mod display;
mod history;
mod memory;
mod paths;
mod protect;
mod sensitive;
mod storage;
mod transfer;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use display::safe;
use std::{env, path::PathBuf, process::ExitCode};

#[derive(Parser, Debug)]
#[command(
    name = "swapctx",
    version,
    about = "Local-first external context memory and manual boundary checks"
)]
struct Cli {
    /// Explicit storage directory. Prefer absolute paths when changing folders.
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Optional managed Codex console; requires Node 24+ and configured Codex 0.162.1.
    Codex {
        #[arg(long)]
        node: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: codex::Action,
    },
    /// Emit a versioned, budgeted JSON context packet through a read-only snapshot.
    Context {
        #[command(flatten)]
        request: context::Request,
    },
    /// Register exactly the current directory as a project.
    Init,
    Status,
    Protect {
        #[command(subcommand)]
        action: protect::Action,
    },
    Memory {
        #[command(subcommand)]
        action: memory::Action,
    },
    Privacy,
    /// Report implemented capabilities without opening the database.
    Capabilities,
    /// Read-only SQLite integrity, foreign-key and lifecycle checks.
    Doctor,
    /// Create a consistent plaintext SQLite snapshot; never overwrite a file.
    Backup {
        destination: PathBuf,
    },
    /// Export only the selected project's context to a portable plaintext SQLite bundle.
    Export {
        destination: PathBuf,
    },
    /// Preview a project bundle; --apply transactionally merges by stable origin IDs.
    Import {
        source: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        allow_sensitive: bool,
    },
    /// Restore a verified whole-store snapshot into a NEW explicit data directory.
    Restore {
        source: PathBuf,
    },
}

fn storage_path(data_dir: Option<PathBuf>) -> Result<PathBuf> {
    let directory = match data_dir {
        Some(path) => {
            if path.as_os_str().is_empty() {
                bail!("Data directory must not be empty");
            }
            if path.has_root() != path.is_absolute()
                || (!path.is_absolute()
                    && path
                        .components()
                        .any(|p| matches!(p, std::path::Component::Prefix(_))))
            {
                bail!("Data directory must be an ordinary relative path or an absolute path");
            }
            if path.is_absolute() {
                path
            } else {
                env::current_dir()?.join(path)
            }
        }
        None => {
            #[cfg(windows)]
            let base = env::var_os("LOCALAPPDATA")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .context("LOCALAPPDATA is missing; supply --data-dir")?;
            #[cfg(not(windows))]
            let base = env::var_os("XDG_DATA_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .or_else(|| {
                    env::var_os("HOME")
                        .filter(|v| !v.is_empty())
                        .map(|h| PathBuf::from(h).join(".local/share"))
                })
                .context("Cannot locate local data directory; supply --data-dir")?;
            if !base.is_absolute() {
                bail!("Local data directory must be absolute; supply --data-dir");
            }
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
    term.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn capabilities() {
    println!("External memory: explicit local SQLite records; no native model context expansion");
    println!("Retrieval: literal substring search, active/history filters, explicit expiry and conflict keys");
    println!(
        "Provenance: self-reported origins and transactional lifecycle history; no tamper proofing"
    );
    println!("Recovery: project SQLite export/import, consistent whole-store backup, restore into a new explicit directory");
    println!("Boundary policies: deny/ask/audit; manual snapshot checks only");
    println!("Enforcement: none; no operation is intercepted and no approval is granted");
    println!("Machine context: swapctx.context v1 JSON; read-only snapshot, byte budgets and mandatory coverage");
    println!("Codex: optional managed app-server console for 0.162.1; context-first exec remains available");
    println!("Lifecycle: startup/resume/manual compaction passed a prior real-model pilot; natural automatic recovery unverified");
    println!("Native Codex hooks: delivery unverified; no automatic memory capture or tool-policy enforcement");
    println!("Hidden model state: inaccessible; model use of delivered records is not guaranteed");
    println!("Sensitive values: limited heuristics and output hiding; storage is plaintext");
    println!("Rust network clients and telemetry: none; optional Codex commands use Codex's configured provider and history");
}

fn run(cli: Cli) -> Result<u8> {
    if let Command::Codex { node, json, action } = cli.command {
        return codex::execute(storage_path(cli.data_dir)?, node, json, action);
    }
    if let Command::Context { request } = cli.command {
        return Ok(context::execute(storage_path(cli.data_dir), request));
    }
    if matches!(cli.command, Command::Capabilities) {
        capabilities();
        return Ok(0);
    }
    let explicit_data_dir = cli.data_dir.is_some();
    let db_path = storage_path(cli.data_dir)?;
    if let Command::Restore { source } = &cli.command {
        // The default store is never a restore target.
        if !explicit_data_dir {
            bail!("Restore requires an explicit --data-dir pointing to a new directory");
        }
        transfer::restore(source, &db_path)?;
        println!(
            "Restored verified snapshot into a new data directory; no existing store was replaced"
        );
        return Ok(0);
    }
    if matches!(cli.command, Command::Privacy) {
        capabilities();
        println!("Storage: {}", safe(&db_path.display().to_string()));
        println!("Data is NOT encrypted at rest. Local account permissions apply.");
        println!("External coding agents retain their own network behavior.");
        println!(
                "Building from source may download dependencies; optional Codex commands can submit model requests."
        );
        return Ok(0);
    }
    let create = matches!(cli.command, Command::Init);
    let writable = create
        || match &cli.command {
            Command::Protect { action } => action.writable(),
            Command::Memory { action } => action.writable(),
            Command::Import { apply, .. } => *apply,
            _ => false,
        };
    let mut conn = storage::open(&db_path, create, writable)?;
    let cwd = env::current_dir()?
        .canonicalize()
        .context("Cannot resolve current directory")?;
    if let Command::Backup { destination } = &cli.command {
        backup::snapshot(&conn, destination)?;
        println!(
            "Verified plaintext backup: {}",
            safe(&destination.display().to_string())
        );
        return Ok(0);
    }
    if create {
        storage::register(&mut conn, &cwd)?;
        println!("Initialized project: {}", safe(paths::path_text(&cwd)?));
        println!(
            "All records stored locally: {}",
            safe(&db_path.display().to_string())
        );
        println!("No agent integration or automatic blocking is enabled.");
        return Ok(0);
    }
    if matches!(cli.command, Command::Doctor) {
        storage::diagnose(&conn)?;
        let sqlite: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
        println!("Database integrity: ok; foreign keys: ok; lifecycle metadata: ok");
        println!(
            "Schema: {} (latest {}) SQLite: {}",
            storage::version(&conn)?,
            storage::SCHEMA_VERSION,
            safe(&sqlite)
        );
        match storage::project(&conn, &cwd) {
            Ok((_, root)) => println!("Project: {}", safe(paths::path_text(&root)?)),
            Err(error) => println!("Project lookup: {}", safe(&format!("{error:#}"))),
        }
        return Ok(0);
    }
    let (id, root) = storage::project(&conn, &cwd)?;
    match cli.command {
        Command::Status => {
            let rules: i64 = conn.query_row(
                "SELECT COUNT(*) FROM rules WHERE project_id = ?1",
                [id],
                |r| r.get(0),
            )?;
            let memories: i64 = conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE project_id = ?1",
                [id],
                |r| r.get(0),
            )?;
            println!("SWAPCTX v{} - LOCAL MODE", env!("CARGO_PKG_VERSION"));
            println!("Project: {}", safe(paths::path_text(&root)?));
            println!("Protected path rules: {rules}");
            println!("Memory records: {memories}");
            println!("Database: {}", safe(&db_path.display().to_string()));
            println!("Schema: {}", storage::version(&conn)?);
            println!("Agent hooks: not connected; enforcement: none");
        }
        Command::Protect { action } => return protect::run(&mut conn, id, &cwd, &root, action),
        Command::Memory { action } => memory::run(&mut conn, id, action)?,
        Command::Export { destination } => transfer::export(&mut conn, id, &destination)?,
        Command::Import {
            source,
            apply,
            allow_sensitive,
        } => transfer::import(&mut conn, id, &source, apply, allow_sensitive)?,
        _ => unreachable!("handled before project lookup"),
    }
    Ok(0)
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!(
                "SwapCtx error: {}",
                sensitive::display(&format!("{error:#}"), false)
            );
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
