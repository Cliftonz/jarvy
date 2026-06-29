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

use super::rules::VersionSource;

pub fn extract_version(project_dir: &Path, source: &VersionSource) -> Option<String> {
    let path = project_dir.join(&source.file);
    let content = std::fs::read_to_string(&path).ok()?;

    if let Some(pattern) = &source.pattern {
        let re = regex::Regex::new(pattern).ok()?;
        let caps = re.captures(&content)?;
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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
}
