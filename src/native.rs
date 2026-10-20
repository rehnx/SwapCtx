//! Native provider UIs. The optional adapter never changes provider settings.
use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
#[cfg(windows)]
use std::path::{Component, Path};
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
    /// Read-only local health check. Never starts or probes a provider process.
    #[arg(long, conflicts_with_all = ["context", "arguments"])]
    pub check: bool,
    /// Absolute native executable or standard npm provider shim.
    #[arg(long)]
    pub executable: Option<PathBuf>,
    /// Absolute Node executable; 24+ for context/check, also used by npm JS entries.
    #[arg(long)]
    pub node: Option<PathBuf>,
    /// Arguments passed unchanged to the original UI, after --.
    #[arg(last = true)]
    pub arguments: Vec<String>,
}

struct Target {
    executable: Option<PathBuf>,
    script: Option<PathBuf>,
}

impl Provider {
    fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

#[cfg(windows)]
fn npm_target(shim: &Path, provider: Provider) -> Result<Target> {
    // Read package metadata, never interpret an arbitrary shell command/shim.
    if !shim
        .file_stem()
        .is_some_and(|s| s.eq_ignore_ascii_case(provider.name()))
    {
        bail!("Not a standard provider shim name");
    }
    let base = shim
        .parent()
        .context("Invalid shim directory")?
        .canonicalize()?;
    let package = base
        .join(match provider {
            Provider::Codex => "node_modules/@openai/codex",
            Provider::Claude => "node_modules/@anthropic-ai/claude-code",
        })
        .canonicalize()?;
    if !package.starts_with(&base) {
        bail!("Package directory escapes its installation prefix");
    }
    let manifest = package.join("package.json").canonicalize()?;
    if !manifest.starts_with(&package) || std::fs::metadata(&manifest)?.len() > 65536 {
        bail!("Invalid package metadata");
    }
    let json = std::fs::read_to_string(manifest)?;
    // Reuse bundled SQLite's JSON parser; no disk database or new dependency.
    let parser = rusqlite::Connection::open_in_memory()?;
    let ambiguous: bool = parser.query_row(
        "SELECT EXISTS(SELECT 1 FROM json_tree(?1) WHERE key IS NOT NULL GROUP BY parent,key HAVING count(*)>1)",
        [&json], |r| r.get(0),
    )?;
    if ambiguous {
        bail!("Ambiguous package metadata");
    }
    let (name, bin): (String, String) = parser.query_row(
        "SELECT json_extract(?1,'$.name'), CASE WHEN json_type(?1,'$.bin')='text' THEN json_extract(?1,'$.bin') ELSE json_extract(?1,?2) END",
        rusqlite::params![json, format!("$.bin.{}", provider.name())],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let expected = match provider {
        Provider::Codex => "@openai/codex",
        Provider::Claude => "@anthropic-ai/claude-code",
    };
    let relative = Path::new(&bin);
    if name != expected
        || bin.contains(':')
        || bin.chars().any(char::is_control)
        || relative.is_absolute()
        || !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        bail!("Invalid npm provider entry point");
    }
    let script = package.join(relative).canonicalize()?;
    if !script.starts_with(&package)
        || !script.is_file()
        || !script.extension().is_some_and(|s| {
            ["js", "cjs", "mjs", "exe"]
                .iter()
                .any(|ext| s.eq_ignore_ascii_case(ext))
        })
    {
        bail!("Unsupported npm provider entry point");
    }
    if script
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
    {
        if script == std::env::current_exe()?.canonicalize()? {
            bail!("Provider resolves to SwapCtx itself");
        }
        return Ok(Target {
            executable: Some(script),
            script: None,
        });
    }
    // Node's main-module resolver rejects Rust's verbatim drive paths. Preserve
    // canonical identity checks above, but pass a conventional Windows path.
    let text = crate::paths::path_text(&script)?;
    let command_path = if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{unc}"))
    } else {
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text))
    };
    Ok(Target {
        executable: None,
        script: Some(command_path),
    })
}

fn provider_target(path: Option<PathBuf>, provider: Provider) -> Result<Target> {
    let direct = executable(path.clone(), provider.name());
    if let Ok(executable) = &direct {
        if executable == &std::env::current_exe()?.canonicalize()? {
            bail!("provider_self_recursion: provider resolves to SwapCtx itself");
        }
        return Ok(Target {
            executable: Some(executable.clone()),
            script: None,
        });
    }
    #[cfg(windows)]
    {
        let cwd = std::env::current_dir()?.canonicalize()?;
        let candidates = if let Some(path) = path {
            vec![path]
        } else {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .filter(|p| p.is_absolute())
                .filter_map(|p| p.canonicalize().ok())
                .filter(|p| p != &cwd)
                .flat_map(|p| {
                    [
                        p.join(format!("{}.cmd", provider.name())),
                        p.join(format!("{}.bat", provider.name())),
                    ]
                })
                .collect()
        };
        for shim in candidates {
            if shim.is_absolute()
                && shim.is_file()
                && shim
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("cmd") || s.eq_ignore_ascii_case("bat"))
            {
                return npm_target(&shim, provider).map_err(|_| anyhow::anyhow!(
                    "unsupported_provider_layout: shell shim has no valid standard npm package entry point. Use an underlying native .exe with --executable, or `swapctx launch {}` for plain trusted shell-shim launching", provider.name()));
            }
        }
    }
    direct.map(|executable| Target {
        executable: Some(executable),
        script: None,
    })
}

pub(crate) fn executable(path: Option<PathBuf>, default: &str) -> Result<PathBuf> {
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
    let provider = options.provider.name();
    let target = provider_target(options.executable, options.provider);
    if options.check {
        println!("Provider resolution: {}", match &target {
            Ok(target) if target.script.is_some() => "ready (standard npm entry point; shim is not executed)",
            Ok(_) => "ready (native executable)",
            Err(error) if error.to_string().starts_with("unsupported_provider_layout:") => "unsupported_provider_layout; install a native .exe or standard npm package (custom shims remain available through plain swapctx launch)",
            Err(error) if error.to_string().starts_with("provider_self_recursion:") => "provider_self_recursion; select the provider, not swapctx.exe",
            Err(_) => "provider_unavailable; install the provider, reopen the terminal or supply --executable ABSOLUTE_PATH",
        });
        println!("Provider compatibility: unverified; no provider process will be started");
    }
    let provider_ready = target.is_ok();
    let target = match target {
        Ok(target) => target,
        Err(_) if options.check => Target {
            executable: None,
            script: None,
        },
        Err(error) => return Err(error),
    };
    let needs_node = options.context || options.check || target.script.is_some();
    let node = if needs_node {
        match executable(options.node, "node") {
            Ok(node) => Some(node),
            Err(_) if options.check => { println!("Node runtime: unavailable; install Node.js 24+ or supply --node ABSOLUTE_PATH"); None },
            Err(error) => return Err(error.context("Node.js is required for this installation/context adapter; supply --node ABSOLUTE_PATH (24+ for context)")),
        }
    } else {
        None
    };
    if !options.context && !options.check {
        let mut command = Command::new(
            target
                .executable
                .or(node)
                .context("Missing provider runtime")?,
        );
        if let Some(script) = target.script {
            command.arg(script);
        }
        let status = command.args(options.arguments).status().context(
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
    let store = crate::storage::open(&database, false, false);
    if options.check {
        println!(
            "Memory store: {}",
            if store.is_ok() {
                "readable"
            } else if !database.exists() {
                "store_missing; use swapctx init at the intended root (same --data-dir)"
            } else {
                "store_unreadable_or_invalid; preserve it and run swapctx doctor"
            }
        );
    }
    let project = store.and_then(|conn| crate::storage::project(&conn, &cwd).map(|(_, root)| root));
    if options.check {
        println!(
            "Project scope: {}",
            if project.is_ok() {
                "registered project and readable store found"
            } else {
                "unavailable; run swapctx init at the intended root, or swapctx doctor for an existing store (use the same --data-dir)"
            }
        );
    }
    let project = project.unwrap_or(cwd);
    let data_dir = database.parent().context("Missing storage parent")?;
    let Some(node) = node else { return Ok(1) };
    let provider_exe = target.executable.unwrap_or_else(|| node.clone());
    let temporary = tempfile::Builder::new()
        .prefix("swapctx-native-")
        .tempdir()?;
    let script = temporary.path().join("native.cjs");
    std::fs::write(&script, include_str!("../integrations/native/native.cjs"))?;
    let mut command = Command::new(node);
    command
        .arg(&script)
        .args([
            if options.check { "check" } else { "launch" },
            "--provider",
            provider,
            "--swapctx",
        ])
        .arg(std::env::current_exe()?.canonicalize()?)
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--project")
        .arg(project)
        .arg("--executable")
        .arg(provider_exe);
    if let Some(script) = target.script {
        command.arg("--provider-script").arg(script);
    }
    let status = command.arg("--").args(options.arguments).status().context(
        "Native context/check adapter requires Node.js 24+; native .exe launching needs no Node",
    )?;
    // Only adapter code and path metadata are temporary; no memory text is stored.
    let code = status
        .code()
        .and_then(|n| u8::try_from(n).ok())
        .unwrap_or(1);
    if let Err(error) = temporary.close() {
        eprintln!(
            "SwapCtx: native adapter temporary cleanup failed; inspect swapctx-native-* in TEMP"
        );
        if code == 0 {
            return Err(error.into());
        }
    }
    Ok(if options.check && !provider_ready {
        1
    } else {
        code
    })
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
                check: false,
                executable: None,
                node: None,
                arguments: Vec::new(),
            },
        )
    })
}
