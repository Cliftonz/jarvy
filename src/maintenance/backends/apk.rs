#![allow(dead_code)] // Linux-only backend; parsers reachable via tests on all OSes.

//! Alpine `apk` freshness backend via `apk search --exact
//! --description-off <pkg>`.
//!
//! Historical `apk search` output shape is a single line per hit:
//!
//! ```text
//! jq-1.7.1-r0
//! ```
//!
//! We strip the trailing `-r<N>` Alpine build revision so the
//! version compare matches upstream semver.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct ApkBackend;

impl FreshnessBackend for ApkBackend {
    fn name(&self) -> &'static str {
        "apk"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        // `-x` restricts to exact match; the modern Alpine
        // toolchain (apk-tools ≥ 2.10) has stable output.
        let probe = crate::tools::common::probe_with_timeout(
            "apk",
            &["search", "-x", pkg_id],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            return Err(BackendError::Other);
        }
        parse_apk_search(pkg_id, &out.stdout)
    }
}

fn parse_apk_search(pkg_id: &str, stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: `<name>-<upstream>-r<revision>`. Split on the
        // LAST `-r<digits>` boundary since names may themselves
        // contain hyphens (e.g. `nats-server`).
        let (head, _rev) = strip_alpine_revision(line);
        // Now find the boundary between name and upstream — the
        // FIRST `-` followed by a digit is upstream's start.
        let (name, upstream) = split_name_from_version(head)?;
        if !name.eq_ignore_ascii_case(pkg_id) {
            continue;
        }
        if upstream.is_empty() {
            return Err(BackendError::ParseFailed);
        }
        return Ok(upstream.to_string());
    }
    Err(BackendError::NotFound)
}

fn strip_alpine_revision(line: &str) -> (&str, Option<&str>) {
    let mut parts = line.rsplitn(2, "-r");
    let last = parts.next().unwrap_or("");
    let head = parts.next();
    match head {
        Some(h) if last.chars().all(|c| c.is_ascii_digit()) => (h, Some(last)),
        _ => (line, None),
    }
}

fn split_name_from_version(head: &str) -> Result<(&str, &str), BackendError> {
    let bytes = head.as_bytes();
    for (i, w) in bytes.windows(2).enumerate() {
        if w[0] == b'-' && w[1].is_ascii_digit() {
            let name = &head[..i];
            let upstream = &head[i + 1..];
            return Ok((name, upstream));
        }
    }
    Err(BackendError::ParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_name() {
        let out = b"jq-1.7.1-r0\n";
        assert_eq!(parse_apk_search("jq", out).unwrap(), "1.7.1");
    }

    #[test]
    fn parses_hyphenated_name() {
        let out = b"nats-server-2.10.14-r0\n";
        assert_eq!(parse_apk_search("nats-server", out).unwrap(), "2.10.14");
    }

    #[test]
    fn wrong_name_is_not_found() {
        let out = b"jq-1.7.1-r0\n";
        assert_eq!(
            parse_apk_search("nope", out).unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn empty_output_is_not_found() {
        assert_eq!(
            parse_apk_search("x", b"").unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn handles_missing_revision() {
        let out = b"jq-1.7.1\n";
        assert_eq!(parse_apk_search("jq", out).unwrap(), "1.7.1");
    }
}
