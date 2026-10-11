//! Saved text is untrusted terminal input, even when stored locally.
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
