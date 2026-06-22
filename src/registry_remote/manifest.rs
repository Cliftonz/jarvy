//! Remote-registry manifest schema.
//!
//! The manifest lists every tool TOML the registry publishes, along with
//! a sha256 of each so Jarvy can refuse a swap-out attack on individual
//! TOML downloads. The manifest itself is cosign-signed (companion
//! `.sig` + `.pem` files at the same URL).
//!
//! Format (JSON):
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "generated_at": "2026-06-22T20:00:00Z",
//!   "tools": [
//!     {
//!       "name": "tailscale-extra",
//!       "path": "tools/tailscale-extra.toml",
//!       "sha256": "abc123..."
//!     }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current manifest schema version. Jarvy refuses to load a manifest
/// claiming a higher version than this — bumping the constant requires a
/// CLI release that knows how to parse the new shape.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("manifest parse failed: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("manifest body is not valid utf-8")]
    InvalidEncoding,
    #[error(
        "manifest schema version {found} is unsupported (expected {supported}); \
         {hint}"
    )]
    UnsupportedSchema {
        found: u32,
        supported: u32,
        hint: &'static str,
    },
    #[error("manifest tool entry {name:?} has invalid path {path:?}: {reason}")]
    InvalidPath {
        name: String,
        path: String,
        reason: &'static str,
    },
    #[error("manifest tool entry {name:?} has invalid sha256 (must be lowercase 64-char hex)")]
    InvalidSha256 { name: String },
    #[error("manifest tool entry {name:?} has invalid name (must match [a-z0-9_-]+)")]
    InvalidName { name: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: Option<String>,
    pub tools: Vec<ToolEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolEntry {
    pub name: String,
    pub path: String,
    pub sha256: String,
}

impl Manifest {
    /// Parse a manifest body. Validates schema version + every entry's
    /// fields before returning — a manifest with one bad row is rejected
    /// wholesale rather than letting partial registration leave the cache
    /// in a half-trusted state.
    pub fn parse(body: &str) -> Result<Self, ManifestError> {
        let manifest: Manifest = serde_json::from_str(body)?;

        // schema_version == 0 is reserved (it's the default for any future
        // sentinel like "draft/do-not-load"); refuse explicitly so the
        // current SUPPORTED_SCHEMA_VERSION isn't accidentally compatible
        // with a zero-valued draft manifest.
        if manifest.schema_version == 0 {
            return Err(ManifestError::UnsupportedSchema {
                found: 0,
                supported: SUPPORTED_SCHEMA_VERSION,
                hint: "schema_version 0 is reserved; the registry must set a positive version",
            });
        }
        if manifest.schema_version > SUPPORTED_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchema {
                found: manifest.schema_version,
                supported: SUPPORTED_SCHEMA_VERSION,
                hint: "upgrade jarvy to use this registry",
            });
        }

        for entry in &manifest.tools {
            validate_name(&entry.name)?;
            validate_path(&entry.name, &entry.path)?;
            validate_sha256(&entry.name, &entry.sha256)?;
        }

        Ok(manifest)
    }
}

/// Allowed tool-name pattern. Mirrors `crate::tools::plugins`'s identifier
/// validation so a remote-synced tool name is always a valid plugin
/// filename stem.
fn validate_name(name: &str) -> Result<(), ManifestError> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Err(ManifestError::InvalidName {
            name: name.to_string(),
        });
    }
    Ok(())
}

/// A manifest path must be a relative reference under the registry root.
/// Refuse:
///
/// - Absolute paths (`/etc/passwd`)
/// - URLs (`https://attacker.example/...`)
/// - Directory traversal (`../../`)
/// - Backslashes (Windows path separators that could confuse extract)
fn validate_path(name: &str, path: &str) -> Result<(), ManifestError> {
    let invalid_reason = if path.is_empty() {
        Some("empty path")
    } else if path.starts_with('/') {
        Some("must be relative, not absolute")
    } else if path.contains("..") {
        Some("must not contain `..` traversal segments")
    } else if path.contains('\\') {
        Some("must not contain backslashes")
    } else if path.contains("://") {
        Some("must be a relative path, not a URL")
    } else if !path.ends_with(".toml") {
        Some("must end in `.toml`")
    } else {
        None
    };

    if let Some(reason) = invalid_reason {
        return Err(ManifestError::InvalidPath {
            name: name.to_string(),
            path: path.to_string(),
            reason,
        });
    }
    Ok(())
}

fn validate_sha256(name: &str, sha: &str) -> Result<(), ManifestError> {
    if sha.len() != 64
        || !sha
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(ManifestError::InvalidSha256 {
            name: name.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_sha() -> &'static str {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }

    #[test]
    fn accepts_minimal_valid_manifest() {
        let body = format!(
            r#"{{
              "schema_version": 1,
              "tools": [
                {{
                  "name": "foo",
                  "path": "tools/foo.toml",
                  "sha256": "{}"
                }}
              ]
            }}"#,
            valid_sha()
        );
        let m = Manifest::parse(&body).expect("should parse");
        assert_eq!(m.tools.len(), 1);
        assert_eq!(m.tools[0].name, "foo");
    }

    #[test]
    fn rejects_newer_schema_version() {
        let body = format!(
            r#"{{"schema_version": 99, "tools": [{{"name": "f", "path": "tools/f.toml", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let err = Manifest::parse(&body).unwrap_err();
        assert!(matches!(
            err,
            ManifestError::UnsupportedSchema { found: 99, .. }
        ));
    }

    #[test]
    fn rejects_schema_version_zero() {
        let body = format!(
            r#"{{"schema_version": 0, "tools": [{{"name": "f", "path": "tools/f.toml", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let err = Manifest::parse(&body).unwrap_err();
        assert!(matches!(
            err,
            ManifestError::UnsupportedSchema { found: 0, .. }
        ));
    }

    #[test]
    fn rejects_path_with_dotdot_traversal() {
        let body = format!(
            r#"{{"schema_version": 1, "tools": [{{"name": "f", "path": "../etc/passwd.toml", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let err = Manifest::parse(&body).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidPath { .. }));
    }

    #[test]
    fn rejects_absolute_path() {
        let body = format!(
            r#"{{"schema_version": 1, "tools": [{{"name": "f", "path": "/etc/passwd.toml", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let err = Manifest::parse(&body).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidPath { .. }));
    }

    #[test]
    fn rejects_url_in_path() {
        let body = format!(
            r#"{{"schema_version": 1, "tools": [{{"name": "f", "path": "https://attacker.example/x.toml", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let err = Manifest::parse(&body).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidPath { .. }));
    }

    #[test]
    fn rejects_uppercase_sha() {
        let upper = valid_sha().to_uppercase();
        let body = format!(
            r#"{{"schema_version": 1, "tools": [{{"name": "f", "path": "tools/f.toml", "sha256": "{}"}}]}}"#,
            upper
        );
        let err = Manifest::parse(&body).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidSha256 { .. }));
    }

    #[test]
    fn rejects_short_sha() {
        let body = r#"{"schema_version": 1, "tools": [{"name": "f", "path": "tools/f.toml", "sha256": "abc"}]}"#;
        let err = Manifest::parse(body).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidSha256 { .. }));
    }

    #[test]
    fn rejects_invalid_tool_name() {
        let body = format!(
            r#"{{"schema_version": 1, "tools": [{{"name": "Bad/Name", "path": "tools/f.toml", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let err = Manifest::parse(&body).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidName { .. }));
    }

    #[test]
    fn rejects_non_toml_path() {
        let body = format!(
            r#"{{"schema_version": 1, "tools": [{{"name": "f", "path": "tools/f.json", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let err = Manifest::parse(&body).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidPath { .. }));
    }

    /// Pin current behavior for `./tools/foo.toml`. The validator
    /// currently accepts a leading `./`. If a future tightening rejects
    /// it, this test moves the assertion side; either way the call site
    /// is documented.
    #[test]
    fn accepts_leading_dot_slash_path() {
        let body = format!(
            r#"{{"schema_version": 1, "tools": [{{"name": "f", "path": "./tools/f.toml", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let m = Manifest::parse(&body).expect("./prefix is currently accepted");
        assert_eq!(m.tools.len(), 1);
    }

    /// Manifest with two entries sharing the same `name`: parse SUCCEEDS
    /// (the validator doesn't dedupe). Last-wins is the sync orchestrator's
    /// problem — the HashMap insert at the loader side resolves. Pin so
    /// that if a future change rejects duplicates, callers know.
    #[test]
    fn accepts_duplicate_tool_names_at_parse_time() {
        let body = format!(
            r#"{{"schema_version": 1, "tools": [
              {{"name": "dup", "path": "tools/dup-a.toml", "sha256": "{}"}},
              {{"name": "dup", "path": "tools/dup-b.toml", "sha256": "{}"}}
            ]}}"#,
            valid_sha(),
            valid_sha()
        );
        let m = Manifest::parse(&body).expect("dedupe is not a parse-layer concern");
        assert_eq!(m.tools.len(), 2);
    }

    /// Very long tool name: regex caps at character set, not length.
    /// Pin that there's no length cap so a future change is intentional.
    #[test]
    fn accepts_arbitrarily_long_name() {
        let long_name = "a".repeat(2048);
        let body = format!(
            r#"{{"schema_version": 1, "tools": [{{"name": "{long_name}", "path": "tools/x.toml", "sha256": "{}"}}]}}"#,
            valid_sha()
        );
        let m = Manifest::parse(&body).expect("no length cap on tool name today");
        assert_eq!(m.tools[0].name.len(), 2048);
    }
}
