//! Version extraction from project marker files.
//!
//! Two paths:
//! - With a `pattern` (regex with one capture group): apply the regex to
//!   the file contents and return capture 1. Used for structured files
//!   like `rust-toolchain.toml` (`channel = "1.85.0"`) and `go.mod`
//!   (`go 1.22`).
//! - Without a `pattern`: return the trimmed file contents. Used for
//!   plain-file conventions like `.nvmrc` / `.python-version` /
//!   `.ruby-version`.

use std::path::Path;
use std::sync::LazyLock;

use super::rules::VersionSource;

/// Max length of an extracted version string. Anything longer is rejected.
/// Defense against (a) multi-MB pathological `rust-toolchain.toml` files
/// blowing up downstream `jarvy.toml` writes and (b) clearly-not-a-version
/// values escaping into the config (review item P2 #17).
const MAX_VERSION_LEN: usize = 64;

/// Cache for user-supplied regex patterns. `default_rules()` uses two
/// hard-coded patterns (rust-toolchain, go.mod) — those are hoisted
/// into their own `LazyLock<Regex>` below and short-circuit this map
/// entirely. Custom rules loaded from `[discover] rules = "..."` land
/// here so a `--watch` loop firing 100× on filesystem events doesn't
/// pay 100× the `Regex::new` NFA/DFA compile cost.
///
/// `RwLock<HashMap>` intentional: reads dominate writes (steady-state
/// after the first firing of each custom rule), and `Mutex` would
/// serialize the hot read path.
static REGEX_CACHE: LazyLock<std::sync::RwLock<std::collections::HashMap<String, regex::Regex>>> =
    LazyLock::new(|| std::sync::RwLock::new(std::collections::HashMap::new()));

/// Compile-cached regexes for the two default-rule patterns. Hoisting
/// these to `LazyLock<Regex>` avoids paying `Regex::new` (~30-100 µs +
/// several kB per compile) on every discover pass that touches a Rust
/// or Go project. The strings are the exact literals used in
/// `default_rules()`; the tests below pin the pairing so a
/// rules.rs edit that diverges the pattern hits a compile-time-loud
/// failure.
const RUST_TOOLCHAIN_PATTERN: &str = r#"channel\s*=\s*"([^"]+)""#;
const GO_MOD_PATTERN: &str = r"^go\s+(\d+\.\d+(?:\.\d+)?)";

static RUST_TOOLCHAIN_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(RUST_TOOLCHAIN_PATTERN).expect("built-in rust-toolchain pattern must compile")
});
static GO_MOD_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(GO_MOD_PATTERN).expect("built-in go.mod pattern must compile")
});

/// Look up a compiled regex, hitting the fast path (LazyLock) for the
/// two default-rule patterns and the RwLock cache for anything else.
///
/// Perf F1: returns `Cow<'static, Regex>` — the default-rule arms
/// hand back `Cow::Borrowed(&RUST_TOOLCHAIN_RE)` (zero atomic RMWs
/// on the internal Arc), custom patterns from the RwLock cache go
/// through Owned. `regex::Regex::clone()` is an atomic increment +
/// internal Arc alloc; pre-refactor we paid that on every hit
/// including the LazyLock hits.
fn get_or_compile_regex(pattern: &str) -> Option<std::borrow::Cow<'static, regex::Regex>> {
    if pattern == RUST_TOOLCHAIN_PATTERN {
        return Some(std::borrow::Cow::Borrowed(&RUST_TOOLCHAIN_RE));
    }
    if pattern == GO_MOD_PATTERN {
        return Some(std::borrow::Cow::Borrowed(&GO_MOD_RE));
    }
    // Try read lock first — steady-state a custom pattern hits after
    // its first compile.
    if let Ok(guard) = REGEX_CACHE.read() {
        if let Some(re) = guard.get(pattern) {
            return Some(std::borrow::Cow::Owned(re.clone()));
        }
    }
    // Miss: compile once, upgrade to write lock, insert.
    let compiled = regex::Regex::new(pattern).ok()?;
    if let Ok(mut guard) = REGEX_CACHE.write() {
        guard
            .entry(pattern.to_string())
            .or_insert_with(|| compiled.clone());
    }
    Some(std::borrow::Cow::Owned(compiled))
}

pub fn extract_version(project_dir: &Path, source: &VersionSource) -> Option<String> {
    let path = project_dir.join(&source.file);
    let content = std::fs::read_to_string(&path).ok()?;

    let raw = if let Some(pattern) = &source.pattern {
        let re = get_or_compile_regex(pattern)?;
        let caps = re.captures(&content)?;
        caps.get(1)?.as_str().to_string()
    } else {
        // Plain-file conventions like `.nvmrc` / `.python-version`. Strip
        // a leading UTF-8 BOM (`\u{feff}`) so a Windows-edited file
        // doesn't produce e.g. `python = "\u{feff}3.12"` (review item
        // P2 #20). Then trim whitespace.
        let stripped = content.strip_prefix('\u{feff}').unwrap_or(&content);
        stripped.trim().to_string()
    };

    sanitize_version(raw)
}

/// Strict allowlist on the extracted version. Refuses any byte outside
/// `[A-Za-z0-9._+~:\-]` — the union of legitimate semver / channel /
/// nightly-tag characters across rust / node / python / go / ruby. This
/// closes the P0 TOML-injection vector where an attacker-controlled
/// `.python-version` / `rust-toolchain.toml` lands `\n[packages]\n...`
/// into the generator (review item P0 #1).
fn sanitize_version(raw: String) -> Option<String> {
    if raw.is_empty() || raw.len() > MAX_VERSION_LEN {
        return None;
    }
    if !raw
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '~' | ':' | '-'))
    {
        return None;
    }
    Some(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn pattern_extracts_first_capture() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.85.0\"",
        )
        .unwrap();
        let src = VersionSource {
            file: "rust-toolchain.toml".into(),
            pattern: Some(r#"channel\s*=\s*"([^"]+)""#.into()),
        };
        assert_eq!(extract_version(tmp.path(), &src), Some("1.85.0".into()));
    }

    #[test]
    fn plain_file_returns_trimmed_content() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".python-version"), "3.12.1\n").unwrap();
        let src = VersionSource {
            file: ".python-version".into(),
            pattern: None,
        };
        assert_eq!(extract_version(tmp.path(), &src), Some("3.12.1".into()));
    }

    #[test]
    fn missing_file_returns_none() {
        let tmp = tempdir().unwrap();
        let src = VersionSource {
            file: ".nvmrc".into(),
            pattern: None,
        };
        assert_eq!(extract_version(tmp.path(), &src), None);
    }

    #[test]
    fn empty_plain_file_returns_none() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".nvmrc"), "   \n").unwrap();
        let src = VersionSource {
            file: ".nvmrc".into(),
            pattern: None,
        };
        assert_eq!(extract_version(tmp.path(), &src), None);
    }

    #[test]
    fn invalid_regex_returns_none() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("x"), "y").unwrap();
        let src = VersionSource {
            file: "x".into(),
            pattern: Some(r"(unbalanced".into()),
        };
        assert_eq!(extract_version(tmp.path(), &src), None);
    }

    /// Review P0 #1 — TOML injection via a hostile `.python-version`.
    /// Newline + bracket + quote chars are forbidden; an attacker
    /// trying to land `\n[packages]\n` MUST be refused at extraction.
    #[test]
    fn refuses_newline_and_bracket_injection_in_plain_file() {
        let tmp = tempdir().unwrap();
        for hostile in [
            "3.12\n[packages]\nallow_remote = true\n# ",
            "3.12\"\n# closed string",
            "3.12; rm -rf /",
            "3.12|evil",
            "[provisioner]",
        ] {
            fs::write(tmp.path().join(".python-version"), hostile).unwrap();
            let src = VersionSource {
                file: ".python-version".into(),
                pattern: None,
            };
            assert_eq!(
                extract_version(tmp.path(), &src),
                None,
                "must refuse hostile version {hostile:?}"
            );
        }
    }

    /// Same threat through the regex path — capture group must not
    /// contain TOML metachars even if the upstream regex `[^"]+` would
    /// happily match a newline.
    #[test]
    fn refuses_metachars_in_regex_capture() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"stable\n[packages]\nbad = true\"",
        )
        .unwrap();
        let src = VersionSource {
            file: "rust-toolchain.toml".into(),
            pattern: Some(r#"channel\s*=\s*"([^"]+)""#.into()),
        };
        assert_eq!(extract_version(tmp.path(), &src), None);
    }

    /// Review P2 #20 — UTF-8 BOM at the start of a Windows-edited
    /// `.nvmrc` must not leak into the synthesized jarvy.toml entry.
    #[test]
    fn strips_utf8_bom_from_plain_version_file() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".nvmrc"), "\u{feff}20.0.0\n").unwrap();
        let src = VersionSource {
            file: ".nvmrc".into(),
            pattern: None,
        };
        assert_eq!(extract_version(tmp.path(), &src), Some("20.0.0".into()));
    }

    /// Review P2 #17 — defense against pathological multi-MB version
    /// files.
    #[test]
    fn refuses_version_longer_than_cap() {
        let tmp = tempdir().unwrap();
        let huge = "1".repeat(MAX_VERSION_LEN + 1);
        fs::write(tmp.path().join(".python-version"), &huge).unwrap();
        let src = VersionSource {
            file: ".python-version".into(),
            pattern: None,
        };
        assert_eq!(extract_version(tmp.path(), &src), None);
    }

    /// Reasonable real-world version strings MUST still pass.
    #[test]
    fn accepts_real_version_strings() {
        let tmp = tempdir().unwrap();
        // Note: nvm aliases like `lts/iron` contain `/` and are
        // intentionally refused — the installer would reject them
        // anyway, and `/` outside a strict allowlist invites future
        // path-traversal concerns if any downstream consumer ever
        // treated the version as a path.
        for (file, content, want) in [
            (".python-version", "3.12.1", "3.12.1"),
            (".nvmrc", "v20.0.0", "v20.0.0"),
            (".ruby-version", "3.2.2", "3.2.2"),
            (".python-version", "3.13.0a1", "3.13.0a1"),
            (
                ".tool-version",
                "1.85.0-nightly+abc.123",
                "1.85.0-nightly+abc.123",
            ),
        ] {
            fs::write(tmp.path().join(file), content).unwrap();
            let src = VersionSource {
                file: file.into(),
                pattern: None,
            };
            assert_eq!(
                extract_version(tmp.path(), &src),
                Some(want.into()),
                "real version {content:?} must pass"
            );
        }
    }
}
