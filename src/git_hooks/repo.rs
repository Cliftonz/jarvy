//! Git-repo source for `[git_hooks.native]`.
//!
//! Clones a hooks repo into a per-URL cache dir, checks out a
//! specific ref, and hands the path back to the native handler so its
//! folder-scan logic can walk the resolved tree. Idempotent: repeat
//! invocations run `git fetch + reset --hard <ref>` instead of a
//! fresh clone.
//!
//! URL grammar mirrors `library_registry::url_parser` (`github:`,
//! `https://`, `git+https://`) with `git+file://` allowed only under
//! `JARVY_LIBRARY_ALLOW_INSECURE_GIT=1` for e2e testing against local
//! bare repos. Ref pinning is REQUIRED — every URL shape needs an
//! explicit `git_ref`; unpinned repos would let a publisher rev
//! silently.

use super::HookError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolved git source ready for cloning. `repo` is always an
/// `https://`, `git+https://`, or (under the insecure-git test env
/// var) `file://` URL that `git clone` accepts directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepo {
    pub repo: String,
    pub git_ref: String,
    pub subpath: Option<String>,
}

/// Errors surfaced when resolving or cloning the repo source.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error(
        "[git_hooks.native] repo `{0}`: URL must start with `github:`, \
         `https://`, or `git+https://`"
    )]
    UnsupportedScheme(String),

    #[error("[git_hooks.native] repo `{0}` is set but `ref` is missing (required)")]
    MissingRef(String),

    #[error(
        "[git_hooks.native] subpath `{0}` is absolute or contains `..`; \
         must be a relative path inside the repo"
    )]
    UnsafeSubpath(String),

    #[error("`git` CLI not found on PATH; install git before using `[git_hooks.native] repo`")]
    GitNotFound,

    #[error("git operation failed: {stage} (exit {exit}): {stderr}")]
    GitCommand {
        stage: &'static str,
        exit: i32,
        stderr: String,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("~/.jarvy/git_hooks_cache/ is unreachable (no HOME)")]
    NoCacheHome,
}

impl From<RepoError> for HookError {
    fn from(err: RepoError) -> Self {
        HookError::Config(err.to_string())
    }
}

/// Resolve `[git_hooks.native] { repo, ref, subpath }` into a
/// [`ResolvedRepo`] with a clone-ready URL. Rejects unpinned repos,
/// unsupported schemes, and unsafe subpaths at parse time.
pub fn resolve(
    repo: &str,
    git_ref: Option<&str>,
    subpath: Option<&str>,
) -> Result<ResolvedRepo, RepoError> {
    let git_ref = git_ref
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| RepoError::MissingRef(repo.to_string()))?
        .to_string();

    if let Some(sp) = subpath
        && (sp.starts_with('/') || sp.split('/').any(|seg| seg == ".."))
    {
        return Err(RepoError::UnsafeSubpath(sp.to_string()));
    }

    let clone_url = if let Some(rest) = repo.strip_prefix("github:") {
        let path = if rest.ends_with(".git") {
            rest.to_string()
        } else {
            format!("{rest}.git")
        };
        format!("https://github.com/{path}")
    } else if let Some(rest) = repo.strip_prefix("git+https://") {
        format!("https://{rest}")
    } else if let Some(rest) = repo.strip_prefix("git+file://") {
        // Loopback-only bypass for e2e tests that clone from a local
        // bare repo in a tempdir. Same env var as `library_registry`
        // so the two subsystems agree.
        if std::env::var_os("JARVY_LIBRARY_ALLOW_INSECURE_GIT").is_none() {
            return Err(RepoError::UnsupportedScheme(repo.to_string()));
        }
        format!("file://{rest}")
    } else if repo.starts_with("https://") {
        repo.to_string()
    } else {
        return Err(RepoError::UnsupportedScheme(repo.to_string()));
    };

    Ok(ResolvedRepo {
        repo: clone_url,
        git_ref,
        subpath: subpath.map(str::to_string),
    })
}

/// Heuristic: does `git_ref` look like a mutable branch? Used to warn
/// operators before they consume a floating ref. SHAs and `v`-prefixed
/// tags are treated as pinned. Copied from `library_registry` — keep
/// in sync.
pub fn is_mutable_ref(git_ref: &str) -> bool {
    let looks_like_sha =
        git_ref.len() >= 7 && git_ref.len() <= 40 && git_ref.chars().all(|c| c.is_ascii_hexdigit());
    if looks_like_sha {
        return false;
    }
    let looks_like_tag = (git_ref.starts_with('v')
        && git_ref.len() > 1
        && git_ref.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
        || git_ref.chars().next().is_some_and(|c| c.is_ascii_digit());
    !looks_like_tag
}

/// Per-URL cache dir under `~/.jarvy/git_hooks_cache/`. Key is
/// `sha256(repo + "\n" + git_ref)` so a ref bump lands in a fresh
/// dir instead of racing an in-flight fetch. Hex-encoded 64-char
/// dirname keeps every cache entry filesystem-safe.
pub fn cache_dir(resolved: &ResolvedRepo) -> Result<PathBuf, RepoError> {
    let base = jarvy_home()?.join("git_hooks_cache");
    let mut hasher = Sha256::new();
    hasher.update(resolved.repo.as_bytes());
    hasher.update(b"\n");
    hasher.update(resolved.git_ref.as_bytes());
    let hex = hex::encode(hasher.finalize());
    Ok(base.join(hex))
}

fn jarvy_home() -> Result<PathBuf, RepoError> {
    // JARVY_TEST_HOME wins under the `test-bypass` feature so e2e
    // tests don't need to touch the user's real ~/.jarvy. Falls back
    // to $HOME/.jarvy in production.
    #[cfg(feature = "test-bypass")]
    if let Some(test_home) = std::env::var_os("JARVY_TEST_HOME") {
        return Ok(PathBuf::from(test_home).join(".jarvy"));
    }
    let home = dirs::home_dir().ok_or(RepoError::NoCacheHome)?;
    Ok(home.join(".jarvy"))
}

/// Ensure the repo is cloned and checked out at `git_ref`. Idempotent:
/// existing cache with a `.git/` inside gets `fetch + reset --hard
/// FETCH_HEAD`; missing cache dir gets a fresh shallow clone. Returns
/// the resolved local path (with `subpath` joined if set) that the
/// native handler's dir-scan should walk.
pub fn ensure_cloned(resolved: &ResolvedRepo) -> Result<PathBuf, RepoError> {
    ensure_git_available()?;
    let cache = cache_dir(resolved)?;
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if cache.join(".git").exists() {
        refresh(&cache, resolved)?;
    } else {
        clone(&cache, resolved)?;
    }
    let scan_root = match &resolved.subpath {
        Some(sp) => cache.join(sp),
        None => cache.clone(),
    };
    if !scan_root.is_dir() {
        return Err(RepoError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "resolved scan root `{}` is not a directory (bad subpath?)",
                scan_root.display()
            ),
        )));
    }
    Ok(scan_root)
}

fn ensure_git_available() -> Result<(), RepoError> {
    let ok = Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        return Err(RepoError::GitNotFound);
    }
    Ok(())
}

fn clone(dest: &Path, resolved: &ResolvedRepo) -> Result<(), RepoError> {
    let dest_str = dest
        .to_str()
        .ok_or_else(|| RepoError::Io(std::io::Error::other("cache path not UTF-8")))?;
    // `--` before user-controlled positional args closes the
    // argv-injection vector against `git clone --upload-pack=...`.
    run_git(
        &[
            "clone",
            "--depth",
            "1",
            "--no-tags",
            "--",
            &resolved.repo,
            dest_str,
        ],
        None,
        "clone",
    )?;
    run_git(
        &["fetch", "--depth", "1", "origin", "--", &resolved.git_ref],
        Some(dest),
        "fetch",
    )?;
    run_git(
        &["checkout", "--detach", "FETCH_HEAD"],
        Some(dest),
        "checkout",
    )?;
    Ok(())
}

fn refresh(dest: &Path, resolved: &ResolvedRepo) -> Result<(), RepoError> {
    // Fetch the pinned ref, fall back to re-clone if fetch fails
    // (e.g. a SHA that isn't reachable from the default branch, or a
    // corrupted local clone from a killed job).
    let fetch = run_git(
        &["fetch", "--depth", "1", "origin", "--", &resolved.git_ref],
        Some(dest),
        "fetch",
    );
    if fetch.is_err() {
        std::fs::remove_dir_all(dest).ok();
        return clone(dest, resolved);
    }
    run_git(
        &["checkout", "--detach", "FETCH_HEAD"],
        Some(dest),
        "checkout",
    )?;
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&Path>, stage: &'static str) -> Result<(), RepoError> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // Redact potential PATs in the URL when it shows up in stderr —
        // mirrors library_registry::git_fetch::run_git.
        let redacted = crate::network::redact_credentials(&stderr).into_owned();
        return Err(RepoError::GitCommand {
            stage,
            exit: output.status.code().unwrap_or(-1),
            stderr: redacted,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_github_shorthand_expands_to_https() {
        let r = resolve("github:acme/hooks", Some("v1.0.0"), None).unwrap();
        assert_eq!(r.repo, "https://github.com/acme/hooks.git");
        assert_eq!(r.git_ref, "v1.0.0");
    }

    #[test]
    fn resolve_github_shorthand_appends_dot_git_when_missing() {
        let r = resolve("github:acme/hooks", Some("main"), None).unwrap();
        assert!(r.repo.ends_with(".git"));
    }

    #[test]
    fn resolve_github_shorthand_preserves_dot_git_when_present() {
        let r = resolve("github:acme/hooks.git", Some("main"), None).unwrap();
        assert_eq!(r.repo, "https://github.com/acme/hooks.git");
    }

    #[test]
    fn resolve_git_https_passthrough() {
        let r = resolve(
            "git+https://gitea.internal/team/hooks.git",
            Some("v1"),
            None,
        )
        .unwrap();
        assert_eq!(r.repo, "https://gitea.internal/team/hooks.git");
    }

    #[test]
    fn resolve_https_passthrough() {
        let r = resolve("https://gitea.internal/team/hooks.git", Some("v1"), None).unwrap();
        assert_eq!(r.repo, "https://gitea.internal/team/hooks.git");
    }

    #[test]
    fn resolve_refuses_http_scheme() {
        let err = resolve("http://insecure/hooks.git", Some("v1"), None).unwrap_err();
        assert!(matches!(err, RepoError::UnsupportedScheme(_)));
    }

    #[test]
    fn resolve_refuses_ssh_scheme() {
        let err = resolve("git@github.com:acme/hooks.git", Some("v1"), None).unwrap_err();
        assert!(matches!(err, RepoError::UnsupportedScheme(_)));
    }

    #[test]
    fn resolve_refuses_missing_ref() {
        let err = resolve("github:acme/hooks", None, None).unwrap_err();
        assert!(matches!(err, RepoError::MissingRef(_)));
    }

    #[test]
    fn resolve_refuses_empty_ref() {
        let err = resolve("github:acme/hooks", Some(""), None).unwrap_err();
        assert!(matches!(err, RepoError::MissingRef(_)));
    }

    #[test]
    fn resolve_refuses_absolute_subpath() {
        let err = resolve("github:acme/hooks", Some("v1"), Some("/etc")).unwrap_err();
        assert!(matches!(err, RepoError::UnsafeSubpath(_)));
    }

    #[test]
    fn resolve_refuses_parent_subpath() {
        let err = resolve("github:acme/hooks", Some("v1"), Some("hooks/../..")).unwrap_err();
        assert!(matches!(err, RepoError::UnsafeSubpath(_)));
    }

    #[test]
    fn resolve_preserves_valid_subpath() {
        let r = resolve("github:acme/hooks", Some("v1"), Some("scripts/hooks")).unwrap();
        assert_eq!(r.subpath.as_deref(), Some("scripts/hooks"));
    }

    #[test]
    fn resolve_refuses_git_file_without_env_var() {
        // Without JARVY_LIBRARY_ALLOW_INSECURE_GIT set, git+file:// is
        // refused as an unsupported scheme. e2e tests explicitly set
        // the env var.
        // Test in isolation from other tests to keep the env-var check honest.
        unsafe {
            std::env::remove_var("JARVY_LIBRARY_ALLOW_INSECURE_GIT");
        }
        let err = resolve("git+file:///tmp/hooks", Some("v1"), None).unwrap_err();
        assert!(matches!(err, RepoError::UnsupportedScheme(_)));
    }

    #[test]
    fn is_mutable_ref_sha_is_pinned() {
        assert!(!is_mutable_ref("abc1234"));
        assert!(!is_mutable_ref("abcdef1234567890abcdef1234567890abcdef12"));
    }

    #[test]
    fn is_mutable_ref_v_tag_is_pinned() {
        assert!(!is_mutable_ref("v1.0.0"));
        assert!(!is_mutable_ref("v2"));
    }

    #[test]
    fn is_mutable_ref_bare_numeric_tag_is_pinned() {
        assert!(!is_mutable_ref("1.2.3"));
    }

    #[test]
    fn is_mutable_ref_branch_is_mutable() {
        assert!(is_mutable_ref("main"));
        assert!(is_mutable_ref("master"));
        assert!(is_mutable_ref("develop"));
        assert!(is_mutable_ref("feature/x"));
    }

    #[test]
    fn cache_dir_is_stable_for_same_input() {
        let r1 = ResolvedRepo {
            repo: "https://github.com/acme/hooks.git".to_string(),
            git_ref: "v1".to_string(),
            subpath: None,
        };
        let r2 = ResolvedRepo {
            repo: "https://github.com/acme/hooks.git".to_string(),
            git_ref: "v1".to_string(),
            subpath: Some("ignored".to_string()),
        };
        // subpath must NOT affect the cache dir — different subpaths of
        // the same clone share the working tree.
        assert_eq!(cache_dir(&r1).unwrap(), cache_dir(&r2).unwrap());
    }

    #[test]
    fn cache_dir_differs_when_ref_changes() {
        let r1 = ResolvedRepo {
            repo: "https://github.com/acme/hooks.git".to_string(),
            git_ref: "v1".to_string(),
            subpath: None,
        };
        let r2 = ResolvedRepo {
            git_ref: "v2".to_string(),
            ..r1.clone()
        };
        assert_ne!(cache_dir(&r1).unwrap(), cache_dir(&r2).unwrap());
    }
}
