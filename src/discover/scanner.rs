//! Project-tree scanner. Resolves a `DetectionPattern::File` value to a
//! concrete file path under `project_dir`, supporting either exact
//! filenames (the common case) or `*.ext` extension globs (terraform,
//! shell config). Bring-your-own glob crate avoided — the patterns we
//! actually need are simple enough that hand-written matching is
//! cheaper than adding a dep.

use std::path::{Path, PathBuf};

/// One-shot cache of the project root directory listing. Built once
/// per `rules::run()` call and passed to every `find_first_match`
/// invocation so N glob rules cost 1 `read_dir` syscall + N O(1)
/// hash lookups, not N syscalls + N O(dir_size) allocations.
///
/// Root-only by design — subdirectory walks would slow detection on
/// large repos and invite false positives from vendored /
/// `node_modules` content. Marker files that matter live at the root
/// by convention.
pub struct RootIndex {
    /// `filename → absolute path` for every regular file at the root.
    /// `OsString` keys avoid a lossy UTF-8 round-trip on paths with
    /// non-UTF-8 bytes (POSIX filesystems allow arbitrary byte
    /// sequences), preserving the ability to reject them via
    /// `sanitize_source` downstream instead of silently misattributing.
    files: std::collections::HashMap<std::ffi::OsString, PathBuf>,
    /// Sorted list of files by name — used for the `*.ext` glob path
    /// so extension matches are deterministic across filesystems
    /// (`read_dir` iteration order is FS-dependent).
    sorted_files: Vec<PathBuf>,
}

impl RootIndex {
    /// Scan `project_dir` once. Returns an empty index if the dir
    /// can't be read — matches the previous `read_dir(...).ok()?`
    /// behavior at every caller.
    pub fn build(project_dir: &Path) -> Self {
        let mut files_by_name: std::collections::HashMap<std::ffi::OsString, PathBuf> =
            std::collections::HashMap::new();
        let mut sorted_files: Vec<PathBuf> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(project_dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                // File-type check first — `is_file` calls stat again;
                // `entry.file_type()` is one syscall on Unix and returns
                // the cached-in-dirent info when the FS supports it.
                let is_file = entry
                    .file_type()
                    .map(|ft| ft.is_file())
                    .unwrap_or_else(|_| path.is_file());
                if !is_file {
                    continue;
                }
                if let Some(name) = path.file_name() {
                    files_by_name.insert(name.to_owned(), path.clone());
                }
                sorted_files.push(path);
            }
        }
        sorted_files.sort();
        Self {
            files: files_by_name,
            sorted_files,
        }
    }

    /// Cache-backed variant of `find_first_match`. Used by
    /// `rules::run()` inside the per-rule loop.
    ///
    /// Returns `&Path` (borrowed from the index) — the caller reads
    /// the filename, no need to own the path. Perf F5: pre-refactor
    /// returned `Option<PathBuf>` which cloned per hit (5-15 clones
    /// per polyglot pass just to drop them after `.file_name()`).
    pub fn find_first_match(&self, pattern: &str) -> Option<&Path> {
        if let Some(ext) = pattern.strip_prefix("*.") {
            for path in &self.sorted_files {
                if path.extension().and_then(|s| s.to_str()) == Some(ext) {
                    return Some(path);
                }
            }
            return None;
        }
        self.files
            .get(std::ffi::OsStr::new(pattern))
            .map(|p| p.as_path())
    }
}

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
///
/// Legacy standalone entry point — the RootIndex-cached path
/// superseded this for production callers. Kept `#[cfg(test)]` so
/// the scanner's own unit tests (below) exercise the same code
/// shape they historically did without dragging dead code into the
/// release binary.
#[cfg(test)]
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

    // -----------------------------------------------------------------
    // RootIndex direct coverage (QA F4). Production runs exclusively
    // through RootIndex — the free-fn tests above cover the legacy
    // #[cfg(test)] shape only. These tests exercise the OsString hash
    // key, the file-type filter, the sorted-list determinism, and the
    // empty-root fallback.
    // -----------------------------------------------------------------

    #[test]
    fn root_index_exact_lookup_matches_written_files() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        fs::write(tmp.path().join("package.json"), "").unwrap();
        let idx = RootIndex::build(tmp.path());
        assert!(idx.find_first_match("Cargo.toml").is_some());
        assert!(idx.find_first_match("package.json").is_some());
        assert!(idx.find_first_match("nonexistent.toml").is_none());
    }

    #[test]
    fn root_index_filters_directories() {
        let tmp = tempdir().unwrap();
        fs::create_dir(tmp.path().join("subdir")).unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        let idx = RootIndex::build(tmp.path());
        assert!(
            idx.find_first_match("subdir").is_none(),
            "RootIndex must filter directories from the file map — \
             `subdir` is a dir, not a file, so exact-name lookup must \
             miss even though `read_dir` returned the entry"
        );
        // Confirm the file still resolves.
        assert!(idx.find_first_match("Cargo.toml").is_some());
    }

    #[test]
    fn root_index_glob_is_deterministic_across_multiple_matches() {
        let tmp = tempdir().unwrap();
        for name in ["z.rockspec", "a.rockspec", "m.rockspec"] {
            fs::write(tmp.path().join(name), "").unwrap();
        }
        let idx = RootIndex::build(tmp.path());
        let first = idx.find_first_match("*.rockspec").unwrap();
        assert_eq!(
            first.file_name().unwrap(),
            "a.rockspec",
            "glob must return lexicographically-smallest match"
        );
        for _ in 0..3 {
            assert_eq!(
                idx.find_first_match("*.rockspec").unwrap(),
                first,
                "repeat calls must be deterministic (sort invariant)"
            );
        }
    }

    #[test]
    fn root_index_empty_dir_yields_empty_matches() {
        let tmp = tempdir().unwrap();
        let idx = RootIndex::build(tmp.path());
        assert!(idx.find_first_match("Cargo.toml").is_none());
        assert!(idx.find_first_match("*.tf").is_none());
    }

    #[test]
    fn root_index_only_dirs_yields_empty() {
        let tmp = tempdir().unwrap();
        for name in ["a", "b", "c"] {
            fs::create_dir(tmp.path().join(name)).unwrap();
        }
        let idx = RootIndex::build(tmp.path());
        assert!(
            idx.find_first_match("a").is_none(),
            "RootIndex indexes files only — a root of pure directories \
             produces an empty index"
        );
    }

    #[test]
    fn root_index_missing_project_dir_yields_empty_index() {
        // `read_dir` on a nonexistent path returns Err — RootIndex
        // must degrade to an empty index, matching pre-refactor
        // `find_first_match` behavior of `.ok()?`.
        let missing = std::path::Path::new("/this/does/not/exist/for/sure");
        let idx = RootIndex::build(missing);
        assert!(idx.find_first_match("Cargo.toml").is_none());
    }
}
