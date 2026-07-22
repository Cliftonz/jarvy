//! Git-clone fetcher for skill libraries (PRD-055).
//!
//! Clones the configured repo + ref into the library cache directory,
//! walks for `SKILL.md` files under the optional subpath, parses their
//! YAML frontmatter, and synthesizes an in-memory `Manifest` so the
//! existing skill installer pipeline works unchanged.
//!
//! # Why shell out to `git`
//!
//! Vendoring libgit2 / git2-rs would bloat binary size + maintenance.
//! `git` is on virtually every dev machine; when it's missing we
//! refuse with a clear error and a hint. The installer / setup path
//! already checks for / installs `git` via the [provisioner] block.
//!
//! # Trust model
//!
//! Inherits PRD-054 trust gates (no library_sources from remote-origin
//! configs). No additional cosign / sha256 layer for git fetches —
//! the trust anchor is the `@<ref>` pin. Pin to a commit SHA for the
//! strongest guarantee; tags + branches are mutable and emit warnings
//! at fetch time.

use super::LibraryError;
use super::manifest::{LibraryItem, LibrarySkillItem, MANIFEST_SCHEMA_VERSION, Manifest};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Outcome of `sync_git` — counts for telemetry plus the resolved
/// publisher slug used in cache organization.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields surface via Debug + structured logging; reserved for `jarvy library show` (PRD-054 phase 6)
pub struct GitSyncReport {
    pub repo: String,
    pub git_ref: String,
    pub subpath: Option<String>,
    pub skills_discovered: usize,
}

/// Clone (or refresh) `repo` at `git_ref`, walk for SKILL.md under
/// `subpath`, synthesize a `Manifest`, and return it. Caller writes the
/// manifest to disk + populates the in-process cache.
///
/// Returns the synthesized manifest plus the cache directory the
/// clone lives in (so callers can persist `manifest.json` next to it).
pub fn sync_git(
    repo: &str,
    git_ref: &str,
    subpath: Option<&str>,
    cache_dir: &Path,
) -> Result<(Manifest, GitSyncReport), LibraryError> {
    ensure_git_available()?;

    // Telemetry gate (review item 7). New library.git.* events were
    // emitted unconditionally; per CLAUDE.md every domain-scoped event
    // must honor `telemetry_gate::is_enabled()` so users who opt out
    // of OTLP don't ship breadcrumbs.
    let telemetry_on = crate::observability::telemetry_gate::is_enabled();

    if is_mutable_ref(git_ref) && telemetry_on {
        tracing::warn!(
            event = "library.git.mutable_ref",
            repo = %crate::network::redact_credentials(repo),
            git_ref = %git_ref,
            advice = "pin to a commit SHA or tag for tamper-evident updates",
        );
    }

    let clone_dir = cache_dir.join("git");
    let started = std::time::Instant::now();
    if !clone_dir.exists() {
        if telemetry_on {
            tracing::info!(
                event = "library.git.clone_started",
                repo = %crate::network::redact_credentials(repo),
                git_ref = %git_ref,
            );
        }
        ensure_parent(&clone_dir)?;
        git_clone_and_checkout(repo, git_ref, &clone_dir)?;
    } else {
        // Already cloned. Refresh by fetching + re-checking-out the ref.
        // Cheap when the ref hasn't moved; correct when it has.
        git_refresh(repo, git_ref, &clone_dir)?;
    }

    let scan_root = match subpath {
        Some(sp) => resolve_subpath_within(&clone_dir, sp)?,
        None => clone_dir.clone(),
    };

    let items = walk_skills(&scan_root)?;
    let skills_discovered = items.len();

    if skills_discovered == 0 {
        eprintln!(
            "  Warning: no SKILL.md files discovered in {}@{}{}.\n  \
             A skill repo must contain `SKILL.md` files with YAML frontmatter \
             (`name`, `version` required).\n  \
             See: https://jarvy.dev/library-registry/ and https://jarvy.dev/skills/",
            crate::network::redact_credentials(repo),
            git_ref,
            subpath.map(|s| format!("#{s}")).unwrap_or_default(),
        );
    }

    let publisher = derive_publisher(repo);
    let manifest = Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        publisher,
        description: format!("Git-fetched library at {git_ref}"),
        homepage: repo.to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        items,
    };

    if telemetry_on {
        tracing::info!(
            event = "library.git.clone_completed",
            repo = %crate::network::redact_credentials(repo),
            git_ref = %git_ref,
            subpath = %subpath.unwrap_or(""),
            skills_discovered,
            duration_ms = started.elapsed().as_millis() as u64,
        );
    }

    let report = GitSyncReport {
        repo: repo.to_string(),
        git_ref: git_ref.to_string(),
        subpath: subpath.map(str::to_string),
        skills_discovered,
    };

    Ok((manifest, report))
}

fn ensure_git_available() -> Result<(), LibraryError> {
    let ok = Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(event = "library.git.missing_git", os = std::env::consts::OS,);
        }
        return Err(LibraryError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "`git` CLI not found on PATH. Install git (e.g. `jarvy setup` with \
             `git = \"latest\"` in [provisioner]) before using git+https:// or \
             github: library_sources.",
        )));
    }
    Ok(())
}

fn is_mutable_ref(git_ref: &str) -> bool {
    // Heuristic: a 7-40 char hex string looks like a commit SHA → immutable.
    let looks_like_sha =
        git_ref.len() >= 7 && git_ref.len() <= 40 && git_ref.chars().all(|c| c.is_ascii_hexdigit());
    if looks_like_sha {
        return false;
    }
    // Tags conventionally start with `v` followed by a digit, or a digit
    // directly. Treat as immutable-by-convention (publishers MAY re-tag,
    // we document this as a risk in PRD-055 but don't warn for it).
    let looks_like_tag = (git_ref.starts_with('v')
        && git_ref.len() > 1
        && git_ref.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
        || git_ref.chars().next().is_some_and(|c| c.is_ascii_digit());
    if looks_like_tag {
        return false;
    }
    // Everything else (main, master, feature/*) is treated as mutable
    // and warned.
    true
}

fn git_clone_and_checkout(repo: &str, git_ref: &str, dest: &Path) -> Result<(), LibraryError> {
    // Strategy: clone shallow (no checkout), then fetch the ref
    // explicitly, then checkout. This handles tags, branches, AND
    // commit SHAs uniformly — `git clone --branch <sha>` doesn't work
    // because --branch only accepts symbolic refs.
    //
    // `--` separators before every user-controlled positional arg
    // (`<repo>`, `<git_ref>`) close the argv-flag-injection vector
    // against `git fetch --upload-pack=...` etc. The url_parser
    // already refuses `-`-prefixed refs at parse time; the `--` here
    // is belt-and-braces.
    let dest_str = dest.to_str().ok_or_else(|| {
        LibraryError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "cache dir path is not valid UTF-8",
        ))
    })?;

    run_git(
        &["clone", "--depth", "1", "--no-tags", "--", repo, dest_str],
        None,
    )?;
    run_git(
        &["fetch", "--depth", "1", "origin", "--", git_ref],
        Some(dest),
    )?;
    run_git(&["checkout", "--detach", "FETCH_HEAD"], Some(dest))?;
    Ok(())
}

fn git_refresh(repo: &str, git_ref: &str, dest: &Path) -> Result<(), LibraryError> {
    // Reset any local state (defensive — there shouldn't be any) then
    // fetch + check out. `--` separator before user-controlled
    // git_ref (see git_clone_and_checkout rationale).
    run_git(
        &["fetch", "--depth", "1", "origin", "--", git_ref],
        Some(dest),
    )
    .or_else(|_| {
        // Fallback: re-clone if fetch fails (e.g. shallow clone
        // limitations on commit SHAs that aren't reachable from
        // the default branch).
        std::fs::remove_dir_all(dest).ok();
        git_clone_and_checkout(repo, git_ref, dest)
    })?;
    run_git(&["checkout", "--detach", "FETCH_HEAD"], Some(dest))?;
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<(), LibraryError> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let output = cmd.output().map_err(LibraryError::Io)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let redacted = crate::network::redact_credentials(stderr.trim());
        // Redact every arg individually before joining for the log —
        // the `<repo>` slot of `git clone` is a user-controlled URL
        // that may carry `user:token@host` credentials. Stderr is
        // already redacted above; argv was not, leaking PATs into
        // `~/.jarvy/logs/jarvy.log` and any OTLP sink. (Review item 6.)
        let redacted_args: Vec<std::borrow::Cow<'_, str>> = args
            .iter()
            .map(|a| crate::network::redact_credentials(a))
            .collect();
        let redacted_args_joined: String = redacted_args
            .iter()
            .map(|c| c.as_ref())
            .collect::<Vec<_>>()
            .join(" ");
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "library.git.clone_failed",
                args = %redacted_args_joined,
                exit = %output.status.code().unwrap_or(-1),
                error = %redacted,
            );
        }
        return Err(LibraryError::Io(std::io::Error::other(format!(
            "git {} failed: {}",
            redacted_args_joined, redacted
        ))));
    }
    Ok(())
}

fn ensure_parent(p: &Path) -> std::io::Result<()> {
    if let Some(parent) = p.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Resolve a subpath inside the clone root, refusing any `..` traversal
/// or absolute-path escape. Mirrors `safety::resolve_within_workspace`
/// from `src/mcp/extended_tools.rs`.
fn resolve_subpath_within(root: &Path, subpath: &str) -> Result<PathBuf, LibraryError> {
    // Defense in depth: the URL parser already refuses `..` and
    // absolute paths, but a fresh check at fetch time is cheap and
    // catches any future caller that constructs `LibrarySource`
    // programmatically.
    if subpath.contains("..") || subpath.starts_with('/') {
        return Err(LibraryError::Parse {
            url: format!("subpath={subpath}"),
            source: serde::de::Error::custom("subpath traversal refused"),
        });
    }
    let trimmed = subpath.trim_end_matches('/').trim_start_matches('/');
    let joined = root.join(trimmed);
    // Canonicalize the clone root + joined path; if either fails or
    // joined escapes root, refuse.
    let canon_root = root.canonicalize().map_err(LibraryError::Io)?;
    let canon_joined = joined.canonicalize().map_err(LibraryError::Io)?;
    if !canon_joined.starts_with(&canon_root) {
        return Err(LibraryError::Parse {
            url: format!("subpath={subpath}"),
            source: serde::de::Error::custom("subpath escapes clone root"),
        });
    }
    Ok(canon_joined)
}

fn walk_skills(root: &Path) -> Result<Vec<LibraryItem>, LibraryError> {
    // Canonicalize the root ONCE so per-file containment checks below
    // compare against a stable absolute path. If the root itself is a
    // symlink, that's resolved here.
    let canon_root = root.canonicalize().map_err(LibraryError::Io)?;
    let mut items = Vec::new();
    walk_dir(&canon_root, &canon_root, &mut items)?;
    Ok(items)
}

fn walk_dir(
    canon_root: &Path,
    dir: &Path,
    items: &mut Vec<LibraryItem>,
) -> Result<(), LibraryError> {
    // Use `symlink_metadata` (no follow) on the entry so a publisher's
    // committed symlink doesn't redirect the walker to /home/$USER/.ssh
    // or similar. P0 fix: previously `is_dir()` followed symlinks and
    // could walk arbitrary paths the publisher pointed at.
    let dir_meta = match std::fs::symlink_metadata(dir) {
        Ok(m) => m,
        Err(e) => return Err(LibraryError::Io(e)),
    };
    if dir_meta.file_type().is_symlink() || !dir_meta.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).map_err(LibraryError::Io)? {
        let entry = entry.map_err(LibraryError::Io)?;
        let path = entry.path();
        // Skip hidden + git internals.
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.starts_with('.')
        {
            continue;
        }
        // Skip symlinks entirely — both for dir traversal and SKILL.md
        // file targets. A symlinked SKILL.md could point at an
        // arbitrary file outside the clone whose contents would then
        // be packaged into the synthesized manifest and written to
        // `~/.{agent}/skills/`. Use file_type from the entry to avoid
        // an extra syscall.
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::info!(
                    event = "library.git.symlink_skipped",
                    path = %path.strip_prefix(canon_root).unwrap_or(&path).display(),
                );
            }
            continue;
        }
        if file_type.is_dir() {
            walk_dir(canon_root, &path, items)?;
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
            // Defense-in-depth: even with the symlink-skip above,
            // canonicalize the SKILL.md path and assert it lives
            // inside the canonical root. Catches edge cases where the
            // entry itself isn't a symlink but its parent was followed
            // before our check (shouldn't happen given the dir walk
            // also skips symlinks, but belt-and-braces).
            let canon_path = match path.canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };
            if !canon_path.starts_with(canon_root) {
                if crate::observability::telemetry_gate::is_enabled() {
                    tracing::warn!(
                        event = "library.git.path_escape_refused",
                        canon_path = %canon_path.display(),
                    );
                }
                continue;
            }
            match build_skill_item(canon_root, &canon_path) {
                Ok(item) => items.push(LibraryItem::Skill(item)),
                Err(reason) => {
                    let rel = canon_path
                        .strip_prefix(canon_root)
                        .unwrap_or(&canon_path)
                        .display();
                    eprintln!(
                        "  Warning: skipped SKILL.md at {rel}: {reason}. \
                         See https://jarvy.dev/library-registry/#skill-item"
                    );
                    if crate::observability::telemetry_gate::is_enabled() {
                        tracing::info!(
                            event = "library.git_skill.skipped",
                            path = %rel,
                            reason = %reason,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn build_skill_item(_root: &Path, path: &Path) -> Result<LibrarySkillItem, String> {
    let body = std::fs::read(path).map_err(|e| format!("read failed: {e}"))?;
    let body_str = String::from_utf8_lossy(&body);
    let (frontmatter, _) = split_frontmatter(&body_str)
        .ok_or("missing YAML frontmatter (expected `---` ... `---` block at top)")?;
    let parsed: SkillFrontmatter =
        serde_yaml::from_str(frontmatter).map_err(|e| format!("frontmatter parse: {e}"))?;
    let name = parsed
        .name
        .ok_or("missing required `name` field in frontmatter")?;
    let version = parsed
        .version
        .ok_or("missing required `version` field in frontmatter")?;
    let supported = parsed.supported_agents.unwrap_or_default();
    let sha = sha256_hex(&body);
    let file_url = format!("file://{}", path.display());
    Ok(LibrarySkillItem {
        name,
        version,
        description: parsed.description.unwrap_or_default(),
        skill_md_url: file_url,
        skill_md_sha256: sha,
        companion_files: Vec::new(),
        supported_agents: supported,
    })
}

/// Strip the leading `---\n...---\n` block. Returns
/// `(frontmatter_yaml, body_markdown)`. None if no frontmatter.
fn split_frontmatter(body: &str) -> Option<(&str, &str)> {
    let body = body.trim_start_matches('\u{feff}');
    let body = body.trim_start();
    let rest = body.strip_prefix("---")?;
    // Find the closing --- on its own line.
    let close = rest.find("\n---")?;
    Some((
        rest[..close].trim_matches(|c: char| c == '\n' || c == '\r'),
        &rest[close + 4..],
    ))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn derive_publisher(repo: &str) -> String {
    // https://github.com/org/repo.git → org/repo
    let stripped = repo.trim_end_matches(".git").trim_start_matches("https://");
    // Drop the host (`github.com/...`) to get a slug.
    if let Some(idx) = stripped.find('/') {
        stripped[idx + 1..].to_string()
    } else {
        stripped.to_string()
    }
}

#[derive(serde::Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    supported_agents: Option<Vec<String>>,
}

/// Read a `file://` URL into bytes. Used by the skills installer when
/// a manifest item's `skill_md_url` came from a git-fetched library.
///
/// P0 — scope refusal. The URL MUST canonicalize to a path inside the
/// library-cache root (`~/.jarvy/library.d/`). A manifest fetched over
/// HTTPS that declares `skill_md_url = "file:///etc/passwd"` would
/// otherwise be honored — the publisher of any trusted library URL
/// could exfiltrate any user-readable file via the agent skill dir.
/// Bounded by `MAX_ITEM_BYTES` so a multi-GB file doesn't OOM.
pub fn read_file_url(url: &str) -> Result<Vec<u8>, LibraryError> {
    use std::io::Read as _;
    let path = url
        .strip_prefix("file://")
        .ok_or_else(|| LibraryError::Parse {
            url: url.to_string(),
            source: serde::de::Error::custom("not a file:// URL"),
        })?;

    // Anchor: canonical library cache root. If the cache hasn't been
    // created (no prior sync), no file:// URL is acceptable.
    let cache_root = match super::cache::cache_root() {
        Ok(root) => root,
        Err(_) => {
            return Err(LibraryError::Parse {
                url: url.to_string(),
                source: serde::de::Error::custom(
                    "file:// rejected: library cache root unavailable",
                ),
            });
        }
    };
    let canon_root = cache_root.canonicalize().map_err(LibraryError::Io)?;
    let canon_path = std::path::Path::new(path)
        .canonicalize()
        .map_err(LibraryError::Io)?;
    if !canon_path.starts_with(&canon_root) {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "library.file_url_refused",
                reason = "outside_cache_root",
            );
        }
        return Err(LibraryError::Parse {
            url: url.to_string(),
            source: serde::de::Error::custom(
                "file:// rejected: path is outside the library cache root",
            ),
        });
    }

    // Bounded read — MAX_ITEM_BYTES matches the HTTPS-fetch cap so the
    // two paths can't be played off against each other (an attacker
    // who controls a publisher manifest can't trick a 50 GB local file
    // into memory).
    let file = std::fs::File::open(&canon_path).map_err(LibraryError::Io)?;
    let mut limited = file.take(super::fetch::MAX_ITEM_BYTES + 1);
    let mut buf = Vec::with_capacity(8 * 1024);
    limited.read_to_end(&mut buf).map_err(LibraryError::Io)?;
    if buf.len() as u64 > super::fetch::MAX_ITEM_BYTES {
        return Err(LibraryError::Parse {
            url: url.to_string(),
            source: serde::de::Error::custom(format!(
                "file:// rejected: body exceeds {} byte cap",
                super::fetch::MAX_ITEM_BYTES
            )),
        });
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn split_frontmatter_extracts_block() {
        let body = "---\nname: foo\nversion: 1.0\n---\n\n# Body\n";
        let (fm, rest) = split_frontmatter(body).unwrap();
        assert!(fm.contains("name: foo"));
        assert!(rest.contains("# Body"));
    }

    #[test]
    fn split_frontmatter_handles_no_frontmatter() {
        let body = "# Just a heading\n";
        assert!(split_frontmatter(body).is_none());
    }

    #[test]
    fn split_frontmatter_tolerates_bom_and_leading_whitespace() {
        let body = "\u{feff}\n  ---\nname: x\nversion: 1.0\n---\nbody\n";
        let (fm, _) = split_frontmatter(body).unwrap();
        assert!(fm.contains("name: x"));
    }

    #[test]
    fn derive_publisher_from_github_url() {
        assert_eq!(
            derive_publisher("https://github.com/anthropics/skills.git"),
            "anthropics/skills"
        );
        assert_eq!(
            derive_publisher("https://gitlab.com/myorg/jarvy-skills"),
            "myorg/jarvy-skills"
        );
    }

    #[test]
    fn is_mutable_ref_recognizes_shas() {
        assert!(!is_mutable_ref("abc1234"));
        assert!(!is_mutable_ref("0123456789abcdef0123456789abcdef01234567"));
    }

    #[test]
    fn is_mutable_ref_recognizes_tags() {
        assert!(!is_mutable_ref("v1.2.0"));
        assert!(!is_mutable_ref("v2.0.0-rc.1"));
        assert!(!is_mutable_ref("1.0.0"));
    }

    #[test]
    fn is_mutable_ref_flags_branches() {
        assert!(is_mutable_ref("main"));
        assert!(is_mutable_ref("master"));
        assert!(is_mutable_ref("feature/something"));
    }

    #[test]
    fn build_skill_item_round_trips_minimal_frontmatter() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: test-skill\nversion: 1.0.0\ndescription: hi\n---\n\n# Body\n",
        )
        .unwrap();
        let item = build_skill_item(tmp.path(), &path).unwrap();
        assert_eq!(item.name, "test-skill");
        assert_eq!(item.version, "1.0.0");
        assert_eq!(item.description, "hi");
        assert!(item.skill_md_url.starts_with("file://"));
        assert_eq!(item.skill_md_sha256.len(), 64);
    }

    #[test]
    fn build_skill_item_rejects_missing_name() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("SKILL.md");
        std::fs::write(&path, "---\nversion: 1.0.0\n---\nbody\n").unwrap();
        let err = build_skill_item(tmp.path(), &path).unwrap_err();
        assert!(err.contains("name"));
    }

    #[test]
    fn build_skill_item_rejects_missing_version() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("SKILL.md");
        std::fs::write(&path, "---\nname: x\n---\nbody\n").unwrap();
        let err = build_skill_item(tmp.path(), &path).unwrap_err();
        assert!(err.contains("version"));
    }

    #[test]
    fn walk_skills_discovers_nested_files() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        std::fs::write(
            tmp.path().join("a/SKILL.md"),
            "---\nname: a-skill\nversion: 1.0\n---\nbody",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("a/b/SKILL.md"),
            "---\nname: b-skill\nversion: 2.0\n---\nbody",
        )
        .unwrap();
        // A non-SKILL.md is ignored.
        std::fs::write(tmp.path().join("a/README.md"), "ignored").unwrap();
        // A hidden dir is skipped.
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        std::fs::write(
            tmp.path().join(".git/SKILL.md"),
            "---\nname: hidden\nversion: 1.0\n---\nbody",
        )
        .unwrap();

        let items = walk_skills(tmp.path()).unwrap();
        assert_eq!(items.len(), 2);
    }

    /// P0 — read_file_url MUST refuse paths outside the library cache
    /// root. Without the anchor, a publisher-controlled manifest can
    /// declare `skill_md_url = "file:///etc/passwd"` and exfiltrate
    /// any user-readable file through the agent skill dir.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn read_file_url_refuses_paths_outside_cache_root() {
        // SAFETY: serial-test gate (`library_env`) ensures no other env-mutating
        // test in this group runs concurrently.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", tempdir().unwrap().path());
        }
        let tmp = tempdir().unwrap();
        let outside_path = tmp.path().join("evil.md");
        std::fs::write(&outside_path, "secret").unwrap();
        let url = format!("file://{}", outside_path.canonicalize().unwrap().display());
        let err = read_file_url(&url).expect_err("must refuse paths outside cache root");
        let msg = format!("{err}");
        assert!(
            msg.contains("outside") || msg.contains("cache root") || msg.contains("library cache"),
            "expected cache-root refusal, got {msg}"
        );
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    /// Happy path — a file:// URL pointing INSIDE the cache root reads
    /// successfully. Ensures the containment check isn't a blanket
    /// refusal of every file:// URL.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn read_file_url_accepts_paths_inside_cache_root() {
        let home = tempdir().unwrap();
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", home.path());
        }
        // Force cache root creation by calling manifest_cache_path
        // (creates the dir tree).
        let cache_root = super::super::cache::cache_root().unwrap();
        let target = cache_root.join("test-skill.md");
        std::fs::write(&target, b"hello inside").unwrap();
        let url = format!("file://{}", target.canonicalize().unwrap().display());
        let bytes = read_file_url(&url).unwrap();
        assert_eq!(bytes, b"hello inside");
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    /// P0 — symlink in cloned repo must be skipped, NOT followed.
    /// Without this, a publisher's `evil-skill -> /home/$USER/.ssh`
    /// symlink causes the walker to descend into ~/.ssh and package
    /// any SKILL.md it finds.
    #[test]
    #[cfg(unix)]
    fn walk_skills_skips_symlinks() {
        let tmp = tempdir().unwrap();
        // Real SKILL.md the walker should find.
        std::fs::create_dir_all(tmp.path().join("real")).unwrap();
        std::fs::write(
            tmp.path().join("real/SKILL.md"),
            "---\nname: real-skill\nversion: 1.0.0\n---\nbody",
        )
        .unwrap();
        // Symlinked dir pointing OUTSIDE the clone — must be skipped.
        let outside = tempdir().unwrap();
        std::fs::create_dir_all(outside.path().join("evil")).unwrap();
        std::fs::write(
            outside.path().join("evil/SKILL.md"),
            "---\nname: evil-skill\nversion: 1.0.0\n---\nshould not be packaged",
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path().join("evil"), tmp.path().join("evil-link"))
            .unwrap();
        // Symlinked SKILL.md inside the clone — must be skipped.
        std::os::unix::fs::symlink(
            outside.path().join("evil/SKILL.md"),
            tmp.path().join("real/SYMLINKED.md"),
        )
        .unwrap();

        let items = walk_skills(tmp.path()).unwrap();
        assert_eq!(
            items.len(),
            1,
            "only the real SKILL.md should land; symlinked entries refused"
        );
        if let LibraryItem::Skill(s) = &items[0] {
            assert_eq!(s.name, "real-skill");
        } else {
            panic!("expected Skill variant");
        }
    }
}
