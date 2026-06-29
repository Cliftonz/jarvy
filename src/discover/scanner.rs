//! Project-tree scanner. Resolves a `DetectionPattern::File` value to a
//! concrete file path under `project_dir`, supporting either exact
//! filenames (the common case) or `*.ext` extension globs (terraform,
//! shell config). Bring-your-own glob crate avoided — the patterns we
//! actually need are simple enough that hand-written matching is
//! cheaper than adding a dep.

use std::path::{Path, PathBuf};

/// Find the first file under `project_dir` matching `pattern`. Returns
/// `None` if nothing matches.
///
/// `pattern` may be:
/// - a bare filename: `"Cargo.toml"` (matches only at project root)
/// - a `*.ext` glob: `"*.tf"` (matches any file at project root)
///
/// Subdirectory walking is intentionally NOT done — every marker file
/// that matters for detection lives at the project root by convention,
/// and walking arbitrary trees would be slow on large repos and
/// invite false positives from vendored / `node_modules` content.
pub fn find_first_match(project_dir: &Path, pattern: &str) -> Option<PathBuf> {
    if let Some(ext) = pattern.strip_prefix("*.") {
        // Collect-then-sort so the returned `source` attribution is
        // stable across filesystems (review item P2 #19 —
        // `read_dir` iteration order is FS-dependent).
        let mut matches: Vec<std::path::PathBuf> = std::fs::read_dir(project_dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some(ext))
            .collect();
        matches.sort();
        return matches.into_iter().next();
    }

    let candidate = project_dir.join(pattern);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn exact_filename_match() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        assert!(find_first_match(tmp.path(), "Cargo.toml").is_some());
        // A name we never wrote MUST miss — guards against the matcher
        // accidentally accepting "any file" when the pattern is wrong.
        assert!(find_first_match(tmp.path(), "nope.txt").is_none());
    }

    #[test]
    fn extension_glob_match() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("main.tf"), "").unwrap();
        let found = find_first_match(tmp.path(), "*.tf").unwrap();
        assert_eq!(found.file_name().unwrap(), "main.tf");
    }

    #[test]
    fn no_match_when_missing() {
        let tmp = tempdir().unwrap();
        assert!(find_first_match(tmp.path(), "Cargo.toml").is_none());
        assert!(find_first_match(tmp.path(), "*.tf").is_none());
    }

    /// Review P2 #19 — same input must produce the same match
    /// regardless of `read_dir` order. Pin via repeated runs (the
    /// sort + first() pattern is what makes this deterministic).
    #[test]
    fn extension_glob_returns_deterministic_match() {
        let tmp = tempdir().unwrap();
        for name in ["b.tf", "a.tf", "c.tf"] {
            fs::write(tmp.path().join(name), "").unwrap();
        }
        let first = find_first_match(tmp.path(), "*.tf").unwrap();
        // The sort guarantees we get `a.tf` (lexicographically smallest).
        assert_eq!(first.file_name().unwrap(), "a.tf");
        // Three more runs — must produce the same answer every time.
        for _ in 0..3 {
            assert_eq!(
                find_first_match(tmp.path(), "*.tf").unwrap(),
                first,
                "extension-glob matcher must be deterministic"
            );
        }
    }

    #[test]
    fn does_not_walk_subdirs() {
        let tmp = tempdir().unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub").join("Cargo.toml"), "").unwrap();
        // Nested Cargo.toml is intentionally NOT discovered — keeps
        // detection fast and avoids vendored / submodule noise.
        assert!(find_first_match(tmp.path(), "Cargo.toml").is_none());
    }
}
