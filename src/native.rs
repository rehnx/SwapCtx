//! Native provider UIs. The optional adapter never changes provider settings.
use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Provider {
    Codex,
    Claude,
}

#[derive(Args, Debug)]
pub struct Options {
    #[arg(value_enum)]
    pub provider: Provider,
    /// Explicitly expose selected project memory to the selected provider.
    #[arg(long)]
    pub context: bool,
    /// Absolute native provider executable (not a shell script).
    #[arg(long)]
    pub executable: Option<PathBuf>,
    /// Absolute Node.js 24+ executable; required only for context integration.
    #[arg(long)]
    pub node: Option<PathBuf>,
    /// Arguments passed unchanged to the original UI, after --.
    #[arg(last = true)]
    pub arguments: Vec<String>,
}

fn executable(path: Option<PathBuf>, default: &str) -> Result<PathBuf> {
    match path {
        Some(path) => {
            if !path.is_absolute() || !path.is_file() {
                bail!("Executable override must be an absolute existing file");
            }
            let path = path.canonicalize()?;
            if cfg!(windows)
                && !path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
            {
                bail!("Use a native .exe; shell launchers are not supported");
            }
            Ok(path)
        }
        None => {
            // Windows process creation also searches the current directory.
            // A project-local codex.exe/node.exe must never win implicitly.
            let name = if cfg!(windows) {
                format!("{default}.exe")
            } else {
                default.to_owned()
            };
            let cwd = std::env::current_dir()?.canonicalize()?;
            for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
                if !directory.is_absolute() {
                    continue;
                }
                let Ok(directory) = directory.canonicalize() else {
                    continue;
                };
                if directory == cwd {
                    continue;
                }
                let candidate = directory.join(&name);
                if candidate.is_file() {
                    let candidate = candidate.canonicalize()?;
                    if cfg!(windows)
                        && !candidate
                            .extension()
                            .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
                    {
                        continue;
                    }
                    return Ok(candidate);
                }
            }
            bail!("Cannot find {name} in an absolute PATH directory; use an explicit executable override")
        }
    }
}

pub fn execute(data_dir: Option<PathBuf>, options: Options) -> Result<u8> {
    let provider = match options.provider {
        Provider::Codex => "codex",
        Provider::Claude => "claude",
    };
    let provider_exe = executable(options.executable, provider)?;
    if !options.context {
        let status = Command::new(provider_exe)
            .args(options.arguments)
            .status()
            .context(
                "Cannot launch native provider; install its native executable or use --executable",
            )?;
        return Ok(status
            .code()
            .and_then(|n| u8::try_from(n).ok())
            .unwrap_or(1));
    }
    let database = crate::storage_path(data_dir)?;
    let cwd = std::env::current_dir()?.canonicalize()?;
    // Capability fallback must still launch when no memory store exists.
    // Supported integrations independently require a valid read-only snapshot.
    let project = crate::storage::open(&database, false, false)
        .ok()
        .and_then(|conn| crate::storage::project(&conn, &cwd).ok())
        .map(|(_, root)| root)
        .unwrap_or(cwd);
    let data_dir = database.parent().context("Missing storage parent")?;
    let node = executable(options.node, "node")?;
    let temporary = tempfile::Builder::new()
        .prefix("swapctx-native-")
        .tempdir()?;
    let script = temporary.path().join("native.cjs");
    std::fs::write(&script, include_str!("../integrations/native/native.cjs"))?;
    let status = Command::new(node)
        .arg(&script)
        .args(["launch", "--provider", provider, "--swapctx"])
        .arg(std::env::current_exe()?.canonicalize()?)
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--project")
        .arg(project)
        .arg("--executable")
        .arg(provider_exe)
        .arg("--")
        .args(options.arguments)
        .status()
        .context("Native context adapter requires Node.js 24+; ordinary launching needs no Node")?;
    // Only adapter code and path metadata are temporary; no memory text is stored.
    temporary
        .close()
        .context("Cannot remove native adapter temporary directory")?;
    Ok(status
        .code()
        .and_then(|n| u8::try_from(n).ok())
        .unwrap_or(1))
}

pub fn menu(data_dir: Option<PathBuf>) -> Result<u8> {
    crate::provider::select_and_launch(|provider| {
        let provider = match provider {
            crate::provider::Provider::Codex => Provider::Codex,
            crate::provider::Provider::Claude => Provider::Claude,
        };
        print!("Expose selected SwapCtx project memory to this provider? [y/N]: ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        execute(
            data_dir,
            Options {
                provider,
                context: answer.trim().eq_ignore_ascii_case("y"),
                executable: None,
                node: None,
                arguments: Vec::new(),
            },
        )
    })
}
