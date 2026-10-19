//! Optional Node/Codex adapter; embedded for a portable single CLI distribution.
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::{path::PathBuf, process::Command};

#[derive(Subcommand, Debug)]
pub enum Action {
    /// Configure the selected project without changing Codex configuration or hooks.
    Setup {
        /// Absolute path to Codex CLI 0.162.1.
        #[arg(long)]
        codex: PathBuf,
        /// Explicit existing Codex home; credentials remain owned by Codex.
        #[arg(long)]
        codex_home: Option<PathBuf>,
        /// Explicitly request Codex's workspace-write sandbox instead of read-only.
        #[arg(long)]
        workspace_write: bool,
        /// Replace adapter settings, preserving a copy of the previous settings.
        #[arg(long)]
        replace: bool,
    },
    /// Check context, version and app-server availability without model requests.
    Doctor,
    /// Inspect saved settings and current context locally, without starting Codex.
    Status,
    /// Observe the opt-in local status snapshot without starting Codex.
    Watch {
        /// Print one snapshot and exit.
        #[arg(long)]
        once: bool,
    },
    /// Remove a validated lock only when its recorded process no longer exists.
    RecoverLock,
    /// Open the managed console, resuming the project's saved managed session.
    Run {
        /// Start a new thread while preserving the previous session reference.
        #[arg(long)]
        new: bool,
        /// Publish a redacted local observatory snapshot for codex watch.
        #[arg(long)]
        observe: bool,
    },
}

pub fn execute(database: PathBuf, node: Option<PathBuf>, json: bool, action: Action) -> Result<u8> {
    let conn = crate::storage::open(&database, false, false)?;
    let cwd = std::env::current_dir()?.canonicalize()?;
    let (_, root) = crate::storage::project(&conn, &cwd)?;
    let data = database
        .parent()
        .context("Cannot identify storage directory")?
        .canonicalize()?;
    let executable = std::env::current_exe()?.canonicalize()?;
    let node = crate::native::executable(node, "node")?;
    let scripts = tempfile::Builder::new()
        .prefix("swapctx-codex-")
        .tempdir()?;
    let script = scripts.path().join("session.cjs");
    std::fs::write(&script, include_bytes!("../integrations/codex/session.cjs"))?;
    std::fs::write(
        scripts.path().join("observatory.cjs"),
        include_bytes!("../integrations/codex/observatory.cjs"),
    )?;
    let mut command = Command::new(node);
    command.arg(&script);
    match action {
        Action::Setup {
            codex,
            codex_home,
            workspace_write,
            replace,
        } => {
            if !codex.is_absolute() {
                bail!("Codex path must be an absolute executable path");
            }
            command
                .arg("setup")
                .arg("--codex")
                .arg(codex.canonicalize()?);
            if let Some(home) = codex_home {
                if !home.is_absolute() || !home.is_dir() {
                    bail!("Codex home must be an absolute existing directory");
                }
                command.arg("--codex-home").arg(home.canonicalize()?);
            }
            if workspace_write {
                command.arg("--workspace-write");
            }
            if replace {
                command.arg("--replace");
            }
        }
        Action::Doctor => {
            command.arg("doctor");
        }
        Action::Status => {
            command.arg("status");
        }
        Action::Watch { once } => {
            command.arg("watch");
            if once {
                command.arg("--once");
            }
        }
        Action::RecoverLock => {
            command.arg("recover-lock");
        }
        Action::Run { new, observe } => {
            command.arg("run");
            if new {
                command.arg("--new");
            }
            if observe {
                command.arg("--observe");
            }
        }
    }
    command
        .arg("--swapctx")
        .arg(executable)
        .arg("--data-dir")
        .arg(data)
        .arg("--project")
        .arg(root);
    if json {
        command.arg("--json");
    }
    // No shell interpolation, user configuration writes, auth reads or network in Rust.
    drop(conn);
    let status = command
        .status()
        .context("Cannot start Node.js; install Node 24+ or supply codex --node ABSOLUTE_PATH")?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1))
}
