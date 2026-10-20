//! Minimal native provider launcher. Deliberately not a context transport or UI replacement.
//! All provider sessions are started by the user, with inherited terminal I/O.
use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Provider {
    Codex,
    #[value(alias = "claude-code")]
    Claude,
}

impl Provider {
    fn binary_name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Choice {
    Provider(Provider),
    Quit,
}

fn choice(input: &str) -> Result<Choice> {
    match input.trim().to_ascii_lowercase().as_str() {
        "1" | "codex" => Ok(Choice::Provider(Provider::Codex)),
        "2" | "claude" | "claude code" | "claude-code" => Ok(Choice::Provider(Provider::Claude)),
        "" | "q" | "quit" | "exit" => Ok(Choice::Quit),
        _ => bail!("Choose 1 (Codex), 2 (Claude Code), or q (quit)"),
    }
}

/// No arguments launches only in a real terminal; piping to it must not launch an agent.
#[allow(dead_code)]
pub fn select_and_launch(launch: impl FnOnce(Provider) -> Result<u8>) -> Result<u8> {
    select_and_launch_with_info(None, launch)
}

/// Interactive provider selection with optional project information display.
pub fn select_and_launch_with_info(
    info: Option<(&Path, i64, i64)>,
    launch: impl FnOnce(Provider) -> Result<u8>,
) -> Result<u8> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("Provider selection needs an interactive terminal; use `swapctx launch codex` or `swapctx launch claude`");
    }
    crate::display::print_banner();
    println!();
    crate::display::print_project_info(info);

    if crate::display::is_color_enabled() {
        println!("  \x1b[1mChoose your coding agent:\x1b[0m\n");
        println!(
            "    \x1b[1;96m[1]\x1b[0m \x1b[1mCodex\x1b[0m        \x1b[90mOpenAI Codex CLI\x1b[0m"
        );
        println!("    \x1b[1;96m[2]\x1b[0m \x1b[1mClaude Code\x1b[0m  \x1b[90mAnthropic Claude Code CLI\x1b[0m");
        println!("    \x1b[90m[q] Quit         Exit SwapCtx\x1b[0m\n");
        print!(
            "  \x1b[1;92m?\x1b[0m \x1b[1mSelect an option \x1b[90m[1-2, q]\x1b[0m\x1b[1m:\x1b[0m "
        );
    } else {
        println!("SwapCtx — choose your coding agent\n");
        println!("  1  Codex");
        println!("  2  Claude Code");
        println!("  q  Quit (or press Enter)\n");
        print!("Select: ");
    }
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    match choice(&answer)? {
        Choice::Quit => {
            if crate::display::is_color_enabled() {
                println!("  \x1b[90mGoodbye!\x1b[0m");
            }
            Ok(0)
        }
        Choice::Provider(provider) => launch(provider),
    }
}

/// Resolve through absolute entries in PATH rather than the current working directory.
/// Never launch a user-supplied arbitrary shell command or parse a command line.
fn find_on_path(provider: Provider, entries: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?.canonicalize().ok()?;
    for directory in entries {
        if !directory.is_absolute() {
            continue;
        }
        let Ok(directory) = directory.canonicalize() else {
            continue;
        };
        // Absolute PATH entries and junction aliases must not make a repository
        // executable an implicit provider. Explicit overrides remain supported.
        if directory == cwd {
            continue;
        }
        #[cfg(windows)]
        let names = [
            format!("{}.exe", provider.binary_name()),
            format!("{}.cmd", provider.binary_name()),
            format!("{}.bat", provider.binary_name()),
        ];
        #[cfg(not(windows))]
        let names = [provider.binary_name().to_owned()];
        for name in names {
            let file = directory.join(name);
            if file.is_file() {
                return Some(file);
            }
        }
    }
    None
}

fn validate_executable(path: &Path) -> Result<()> {
    if !path.is_absolute() || !path.is_file() {
        bail!("Provider executable must be an absolute path to an existing file");
    }
    #[cfg(windows)]
    {
        let allowed = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                value.eq_ignore_ascii_case("exe")
                    || value.eq_ignore_ascii_case("cmd")
                    || value.eq_ignore_ascii_case("bat")
            });
        if !allowed {
            bail!("Windows provider launcher requires a .exe, .cmd, or .bat executable");
        }
    }
    let target = fs::canonicalize(path).context("Cannot resolve provider executable")?;
    let self_exe = fs::canonicalize(env::current_exe()?)?;
    if target == self_exe {
        bail!("Provider executable resolves to SwapCtx itself; refusing recursive launch");
    }
    Ok(())
}

/// Direct native terminal handoff: no forced sandbox, no replacement UI, no config edits,
/// no project files or database writes, and no hidden model calls by SwapCtx.
pub fn launch(provider: Provider, executable: Option<PathBuf>) -> Result<u8> {
    let path = match executable {
        Some(path) => path,
        None => {
            let paths = env::var_os("PATH")
                .map(|p| env::split_paths(&p).collect::<Vec<_>>())
                .unwrap_or_default();
            find_on_path(provider, paths).with_context(|| {
                format!(
                    "{} executable not found on PATH. Install it, reopen your terminal, or use `swapctx launch {} --executable ABSOLUTE_PATH`",
                    provider.label(),
                    provider.binary_name()
                )
            })?
        }
    };
    validate_executable(&path)?;
    eprintln!("SwapCtx: launching native {} interface. Context is not automatically injected; no memory capture or enforcement is active.", provider.label());
    let exit = Command::new(&path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to start {} native CLI", provider.label()))?;
    Ok(exit
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_selection_is_explicit() {
        assert_eq!(choice("1\n").unwrap(), Choice::Provider(Provider::Codex));
        assert_eq!(choice("Codex").unwrap(), Choice::Provider(Provider::Codex));
        assert_eq!(choice("2").unwrap(), Choice::Provider(Provider::Claude));
        assert_eq!(
            choice("claude-code").unwrap(),
            Choice::Provider(Provider::Claude)
        );
        assert_eq!(choice("q").unwrap(), Choice::Quit);
        assert_eq!(choice("").unwrap(), Choice::Quit);
        assert!(choice("3").is_err());
    }

    #[test]
    fn path_search_does_not_consult_cwd_or_relative_directories() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        #[cfg(windows)]
        let name = "codex.cmd";
        #[cfg(not(windows))]
        let name = "codex";
        fs::write(second.join(name), "fixture").unwrap();
        assert_eq!(
            find_on_path(
                Provider::Codex,
                vec![PathBuf::from("relative"), first, second.clone()],
            ),
            Some(second.canonicalize().unwrap().join(name))
        );
        assert!(find_on_path(Provider::Claude, vec![second]).is_none());
    }

    #[test]
    fn rejects_invalid_override_and_self_recursion() {
        assert!(validate_executable(Path::new("codex")).is_err());
        assert!(validate_executable(Path::new("/this-provider-file-should-not-exist")).is_err());
        assert!(validate_executable(&env::current_exe().unwrap()).is_err());
    }

    #[test]
    fn launches_only_a_local_fake_and_propagates_its_exit_code() {
        let root = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        let mock = root.path().join("mock.cmd");
        #[cfg(not(windows))]
        let mock = root.path().join("mock");
        #[cfg(windows)]
        fs::write(&mock, "@echo off\r\nexit /b 7\r\n").unwrap();
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::write(&mock, "#!/bin/sh\nexit 7\n").unwrap();
            fs::set_permissions(&mock, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert_eq!(launch(Provider::Codex, Some(mock)).unwrap(), 7);
    }
}
