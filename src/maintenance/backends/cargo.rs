//! crates.io freshness backend via `cargo search`.
//!
//! `cargo search <crate> --limit 1` emits one line per hit in the
//! form `<crate> = "<ver>"  # <description>`. Stable format since
//! Cargo 1.0.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct CargoBackend;

impl FreshnessBackend for CargoBackend {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "cargo",
            &["search", pkg_id, "--limit", "1"],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            return Err(BackendError::Other);
        }
        parse_cargo_search(pkg_id, &out.stdout)
    }
}

fn parse_cargo_search(pkg_id: &str, stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("...") {
            continue;
        }
        // Expected shape: `pkg_id = "1.2.3"    # description...`
        let Some((name_part, rest)) = line.split_once('=') else {
            continue;
        };
        if name_part.trim() != pkg_id {
            continue;
        }
        let rest = rest.trim();
        let ver = rest
            .strip_prefix('"')
            .and_then(|s| s.split('"').next())
            .ok_or(BackendError::ParseFailed)?;
        if ver.is_empty() {
            return Err(BackendError::ParseFailed);
        }
        return Ok(ver.to_string());
    }
    Err(BackendError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_top_line() {
        let out =
            b"serde = \"1.0.203\"          # A generic serialization/deserialization framework\n";
        assert_eq!(parse_cargo_search("serde", out).unwrap(), "1.0.203");
    }

    #[test]
    fn ignores_note_line() {
        let out = b"... and 1000 crates more (use --limit N to see more)\n\
                    ripgrep = \"14.1.0\"    # Fast recursive search\n";
        assert_eq!(parse_cargo_search("ripgrep", out).unwrap(), "14.1.0");
    }

    #[test]
    fn wrong_name_yields_not_found() {
        let out = b"serde = \"1.0.203\"    # framework\n";
        assert_eq!(
            parse_cargo_search("tokio", out).unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn empty_output_is_not_found() {
        assert_eq!(
            parse_cargo_search("anything", b"").unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn malformed_line_is_parse_failed() {
        let out = b"serde = notquoted\n";
        assert_eq!(
            parse_cargo_search("serde", out).unwrap_err(),
            BackendError::ParseFailed
        );
    }
}
