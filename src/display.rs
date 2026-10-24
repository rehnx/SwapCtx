//! Saved text is untrusted terminal input, even when stored locally.
use std::io::IsTerminal;
use std::path::Path;
use std::sync::OnceLock;

/// Shell names do not describe the console attached to a process. Probe the
/// actual output handle, and keep the styled UI only when all its features work.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TerminalCapabilities {
    pub interactive: bool,
    pub ansi: bool,
    pub unicode: bool,
    pub no_color: bool,
}

impl TerminalCapabilities {
    pub fn styled(self) -> bool {
        self.interactive && self.ansi && self.unicode && !self.no_color
    }
}

pub(crate) fn terminal() -> TerminalCapabilities {
    static CAPABILITIES: OnceLock<TerminalCapabilities> = OnceLock::new();
    *CAPABILITIES.get_or_init(|| {
        let output_terminal = std::io::stdout().is_terminal();
        let dumb = std::env::var_os("TERM").is_some_and(|value| value == "dumb");
        let (ansi, unicode) = if output_terminal && !dumb {
            platform_capabilities()
        } else {
            (false, false)
        };
        TerminalCapabilities {
            interactive: output_terminal && std::io::stdin().is_terminal(),
            ansi,
            unicode,
            // Presence, including an empty value, disables the styled UI.
            no_color: std::env::var_os("NO_COLOR").is_some(),
        }
    })
}

#[cfg(not(windows))]
fn platform_capabilities() -> (bool, bool) {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase();
    (true, locale.contains("utf-8") || locale.contains("utf8"))
}

#[cfg(windows)]
fn platform_capabilities() -> (bool, bool) {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(kind: u32) -> *mut c_void;
        fn GetConsoleMode(handle: *mut c_void, mode: *mut u32) -> i32;
        fn SetConsoleMode(handle: *mut c_void, mode: u32) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11_i32 as u32;
    const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    const ANSI_OUTPUT: u32 = ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
    // SAFETY: the handle is borrowed from Windows, the mode pointer is valid,
    // and no handle ownership is transferred. Fail closed on unsupported hosts.
    let ansi = unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return (false, false);
        }
        mode & ANSI_OUTPUT == ANSI_OUTPUT || SetConsoleMode(handle, mode | ANSI_OUTPUT) != 0
    };
    // Fonts cannot be reliably inferred from a shell or code page. Windows
    // Terminal provides Unicode rendering; unknown legacy hosts use ASCII even
    // when VT can be enabled. We never change the user's code page or font.
    // Rust writes Unicode to console handles with WriteConsoleW, independently
    // of the active OEM code page (including CMD's usual 437/850).
    let unicode = std::env::var_os("WT_SESSION").is_some_and(|s| !s.is_empty());
    (ansi, unicode)
}

/// Slant-style large ASCII art text for SwapCtx.
pub const BANNER: &str = r#"   _____                     ________
  / ___/      ______ _____  / ____/ /__  __
  \__ \ | /| / / __ `/ __ \/ /   / __/ |/_/
 ___/ / |/ |/ / /_/ / /_/ / /___/ /__>  <
/____/|__/|__/\__,_/ .___/\____/\__/_/|_|
                  /_/"#;

/// Check if ANSI escape colors should be enabled.
pub fn is_color_enabled() -> bool {
    terminal().styled()
}

/// Keep paths legible on limited consoles without changing stored text/JSON.
fn safe_ui(text: &str) -> String {
    let text = safe(text);
    if terminal().styled() {
        text
    } else {
        ascii_ui(&text)
    }
}

fn ascii_ui(text: &str) -> String {
    let mut output = String::new();
    for c in text.chars() {
        if c.is_ascii() {
            output.push(c);
        } else {
            output.extend(c.escape_default());
        }
    }
    output
}

/// Print the styled large SwapCtx text banner and tagline.
pub fn print_banner() {
    if is_color_enabled() {
        println!("\x1b[1;96m{}\x1b[0m", BANNER);
        println!(
            "  \x1b[90mPersistent Context Memory for AI Coding Agents \x1b[1;94m·\x1b[90m v{}\x1b[0m",
            env!("CARGO_PKG_VERSION")
        );
        println!("  \x1b[90m────────────────────────────────────────────────────────────\x1b[0m");
    } else {
        println!("{}", BANNER);
        println!(
            "  Persistent Context Memory for AI Coding Agents - v{}",
            env!("CARGO_PKG_VERSION")
        );
        println!("  ------------------------------------------------------------");
    }
}

/// Format a Windows or Unix path cleanly for display,
/// removing extended path prefixes.
pub fn clean_path_for_display(path: &Path) -> String {
    let s = path.display().to_string();

    if let Some(unc) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

/// Print project scope and memory status overview for interactive menus.
pub fn print_project_info(info: Option<(&Path, i64, i64)>) {
    if let Some((root, memories, rules)) = info {
        let path_str = clean_path_for_display(root);

        if is_color_enabled() {
            println!(
                "  \x1b[1;92m●\x1b[0m \x1b[1mProject:\x1b[0m   \x1b[36m{}\x1b[0m",
                safe_ui(&path_str)
            );
            println!(
                "  \x1b[1;92m●\x1b[0m \x1b[1mStatus:\x1b[0m    \x1b[32mRegistered\x1b[0m \x1b[90m({} memory record{}, {} boundary rule{})\x1b[0m\n",
                memories,
                if memories == 1 { "" } else { "s" },
                rules,
                if rules == 1 { "" } else { "s" }
            );
        } else {
            println!("  * Project:   {}", safe_ui(&path_str));
            println!(
                "  * Status:    Registered ({} memory record{}, {} boundary rule{})\n",
                memories,
                if memories == 1 { "" } else { "s" },
                rules,
                if rules == 1 { "" } else { "s" }
            );
        }
    } else if let Ok(cwd) = std::env::current_dir() {
        let path_str = clean_path_for_display(&cwd);

        if is_color_enabled() {
            println!(
                "  \x1b[90m○\x1b[0m \x1b[1mDirectory:\x1b[0m \x1b[36m{}\x1b[0m \x1b[90m(unregistered)\x1b[0m",
                safe_ui(&path_str)
            );
            println!(
                "  \x1b[90m○\x1b[0m \x1b[90mTip: Run \x1b[1m'swapctx init'\x1b[0m\x1b[90m to enable persistent memory for this project\x1b[0m\n"
            );
        } else {
            println!("  o Directory: {} (unregistered)", safe_ui(&path_str));
            println!("  o Tip: Run 'swapctx init' to enable persistent memory for this project\n");
        }
    }
}

/// Escape untrusted terminal control characters.
pub fn safe(text: &str) -> String {
    let mut output = String::with_capacity(text.len());

    for c in text.chars() {
        if c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
            output.extend(c.escape_default());
        } else {
            output.push(c);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_terminal_controls_but_keeps_unicode() {
        assert_eq!(safe("hello\x1b[2J\n世界"), "hello\\u{1b}[2J\\n世界");
        assert_eq!(safe("\u{202e}"), "\\u{202e}");
    }

    #[test]
    fn styling_requires_every_capability_and_respects_no_color() {
        for bits in 0..16 {
            let caps = TerminalCapabilities {
                interactive: bits & 1 != 0,
                ansi: bits & 2 != 0,
                unicode: bits & 4 != 0,
                no_color: bits & 8 != 0,
            };
            assert_eq!(caps.styled(), bits == 7, "capabilities: {caps:?}");
        }
    }

    #[test]
    fn ascii_paths_preserve_separators_and_escape_unicode_and_controls() {
        assert_eq!(
            ascii_ui(&safe("C:\\My Project\\世界\x1b")),
            "C:\\My Project\\\\u{4e16}\\u{754c}\\u{1b}"
        );
    }
}
