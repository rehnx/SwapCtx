//! Saved text is untrusted terminal input, even when stored locally.
use std::io::IsTerminal;
use std::path::Path;

/// Slant-style large ASCII art text for SwapCtx.
pub const BANNER: &str = r#"   _____                     ________
  / ___/      ______ _____  / ____/ /__  __
  \__ \ | /| / / __ `/ __ \/ /   / __/ |/_/
 ___/ / |/ |/ / /_/ / /_/ / /___/ /__>  <
/____/|__/|__/\__,_/ .___/\____/\__/_/|_|
                  /_/"#;

/// Check if ANSI escape colors should be enabled.
pub fn is_color_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
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
                safe(&path_str)
            );
            println!(
                "  \x1b[1;92m●\x1b[0m \x1b[1mStatus:\x1b[0m    \x1b[32mRegistered\x1b[0m \x1b[90m({} memory record{}, {} boundary rule{})\x1b[0m\n",
                memories,
                if memories == 1 { "" } else { "s" },
                rules,
                if rules == 1 { "" } else { "s" }
            );
        } else {
            println!("  * Project:   {}", safe(&path_str));
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
                safe(&path_str)
            );
            println!(
                "  \x1b[90m○\x1b[0m \x1b[90mTip: Run \x1b[1m'swapctx init'\x1b[0m\x1b[90m to enable persistent memory for this project\x1b[0m\n"
            );
        } else {
            println!("  o Directory: {} (unregistered)", safe(&path_str));
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
}
