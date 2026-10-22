//! Conservative heuristics, not a complete secret detector or encryption.
use anyhow::{bail, Result};

pub fn detected(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("-----begin") && lower.contains("private key-----") {
        return true;
    }
    if [
        "api_key",
        "apikey",
        "api-key",
        "password",
        "passwd",
        "client_secret",
        "access_token",
        "secret_key",
    ]
    .iter()
    .any(|label| {
        lower.match_indices(label).any(|(i, name)| {
            let after = lower[i + name.len()..].trim_start_matches([' ', '\t', '"', '\'']);
            after.starts_with(['=', ':'])
                && after[1..]
                    .trim_matches([' ', '\t', '"', '\'', '\n', '\r'])
                    .len()
                    >= 4
        })
    }) {
        return true;
    }
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .any(|word| {
            ((word.starts_with("AKIA") || word.starts_with("ASIA"))
                && word.len() == 20
                && word
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
                || (word.starts_with("sk-") && word.len() >= 24)
                || ((word.starts_with("ghp_")
                    || word.starts_with("gho_")
                    || word.starts_with("github_pat_"))
                    && word.len() >= 32)
        })
}

pub fn guard(text: &str, allow: bool) -> Result<()> {
    if !allow && detected(text) {
        bail!("Potential sensitive value detected. Remove/redact it or explicitly use --allow-sensitive to store plaintext. The value is not echoed.");
    }
    Ok(())
}

pub fn display(text: &str, reveal: bool) -> String {
    if !reveal && detected(text) {
        "[potential sensitive value hidden]".into()
    } else {
        crate::display::safe(text)
    }
}

/// Redact values for a JSON encoder without changing non-sensitive identities.
pub fn redacted(text: &str) -> &str {
    if detected(text) {
        "[potential sensitive value hidden]"
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_markers_are_detected_without_echoing_values() {
        for text in [
            "PASSWORD=example123",
            "\"api_key\": \"example123\"",
            "sk-abcdefghijklmnopqrstuvwxyz",
            "AKIAABCDEFGHIJKLMNOP",
            "-----BEGIN RSA PRIVATE KEY-----",
        ] {
            assert!(detected(text));
            assert!(guard(text, false).is_err());
            assert!(!display(text, false).contains(text));
            assert!(guard(text, true).is_ok());
        }
        assert!(!detected(
            "Protect authentication and never store passwords"
        ));
    }
}
