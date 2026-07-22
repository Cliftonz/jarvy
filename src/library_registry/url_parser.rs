//! URL-scheme parser for `library_sources.url` (PRD-054 + PRD-055).
//!
//! Three shapes today:
//!
//! - `https://...` — manifest URL (PRD-054 default path)
//! - `git+https://host/owner/repo.git@<ref>[#<subpath>]` — git source (PRD-055)
//! - `github:owner/repo@<ref>[#<subpath>]` — shorthand for the GitHub case
//!
//! `@<ref>` is **required** for git sources. Unpinned URLs (no `@`) are
//! refused at parse time — silent floating refs would let a publisher
//! rev a skill without bumping any pin the consumer can see.

use super::LibraryError;

/// Resolved view of a `library_sources.url` after scheme + path parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceScheme {
    /// `https://...` — fetch a manifest.json directly.
    Manifest { url: String },
    /// `git+https://...@<ref>[#<subpath>]` or `github:org/repo@<ref>[#<subpath>]`.
    Git {
        /// Clone URL passed to `git clone` (always `https://...`).
        repo: String,
        /// Tag / branch / commit SHA.
        git_ref: String,
        /// Optional path inside the repo to scan for SKILL.md.
        subpath: Option<String>,
    },
}

/// Parse a `library_sources.url` value into a typed `SourceScheme`.
/// HTTPS-only at this layer; `http://` and any other scheme is refused
/// upstream by `fetch_bounded` / `git_fetch`.
pub fn parse_source(url: &str) -> Result<SourceScheme, LibraryError> {
    if let Some(rest) = url.strip_prefix("git+https://") {
        return parse_git(&format!("https://{rest}"), url);
    }
    if let Some(rest) = url.strip_prefix("git+file://") {
        // Loopback-test-only bypass mirroring `fetch::insecure_loopback_allowed`.
        // Production users have no way to enable this; integration + E2E
        // tests opt in via JARVY_LIBRARY_ALLOW_INSECURE_GIT=1 plus a
        // local repo path. Anything else is refused as a typo / hostile
        // URL the same way `http://` is refused for manifests.
        if std::env::var_os("JARVY_LIBRARY_ALLOW_INSECURE_GIT").is_none() {
            return Err(LibraryError::Parse {
                url: url.to_string(),
                source: serde::de::Error::custom(
                    "git+file:// is refused in production; \
                     set JARVY_LIBRARY_ALLOW_INSECURE_GIT=1 for local tests only",
                ),
            });
        }
        return parse_git(&format!("file://{rest}"), url);
    }
    if let Some(rest) = url.strip_prefix("github:") {
        // github:owner/repo@ref#subpath → https://github.com/owner/repo.git@ref#subpath
        let (path, suffix) = split_suffix(rest);
        if suffix.is_empty() {
            return Err(LibraryError::Parse {
                url: url.to_string(),
                source: serde::de::Error::custom(
                    "github: shorthand requires an @<ref> pin, e.g. \
                     github:owner/repo@v1.0.0",
                ),
            });
        }
        // Ensure `.git` suffix on the repo path for git compatibility.
        let repo_path = if path.ends_with(".git") {
            path.to_string()
        } else {
            format!("{path}.git")
        };
        let canonical = format!("https://github.com/{repo_path}{suffix}");
        return parse_git(&canonical, url);
    }
    if url.starts_with("https://") {
        return Ok(SourceScheme::Manifest {
            url: url.to_string(),
        });
    }
    Err(LibraryError::Parse {
        url: url.to_string(),
        source: serde::de::Error::custom(
            "library_sources.url must start with https:// (manifest), \
             git+https:// (git clone), or github: (GitHub shorthand)",
        ),
    })
}

/// Parse the `<repo>@<ref>[#<subpath>]` tail of a git URL.
fn parse_git(https_url: &str, original_url: &str) -> Result<SourceScheme, LibraryError> {
    // Strip fragment first (subpath).
    let (head, subpath) = match https_url.find('#') {
        Some(idx) => (&https_url[..idx], Some(https_url[idx + 1..].to_string())),
        None => (https_url, None),
    };

    // Reject suspicious subpaths up front. `..` segments would let a
    // hostile URL escape the cache root when walked. Absolute paths
    // would bypass the clone-root anchor entirely. Empty string is
    // tolerated (parsed as None at this depth).
    if let Some(ref sp) = subpath
        && (sp.starts_with('/') || sp.split('/').any(|seg| seg == ".."))
    {
        return Err(LibraryError::Parse {
            url: original_url.to_string(),
            source: serde::de::Error::custom(
                "git source subpath must be relative and contain no `..` segments",
            ),
        });
    }

    // Split repo and ref on the LAST '@'. URLs can legitimately contain
    // earlier '@' in userinfo (`https://user@host/...`), but our HTTPS
    // gate refuses userinfo upstream — still, defensive: scan from the
    // right.
    let (repo, git_ref) = match head.rsplit_once('@') {
        Some((repo, git_ref)) if !git_ref.is_empty() => (repo, git_ref),
        _ => {
            return Err(LibraryError::Parse {
                url: original_url.to_string(),
                source: serde::de::Error::custom(
                    "git source requires an @<ref> pin (tag / branch / commit SHA); \
                     unpinned refs are refused so silent updates can't ship through",
                ),
            });
        }
    };

    // P0 — argv-flag injection refusal. `git fetch origin <ref>` honors
    // `--upload-pack=cmd` / `--receive-pack` / `--exec` etc. as `<ref>`
    // and runs `cmd` locally during transport setup. A ref starting
    // with `-` is never a legitimate tag/branch/SHA — refuse at parse
    // time. Also refuse whitespace and control bytes (defense in depth
    // against shell-meta smuggling via ANSI escapes in error messages).
    if git_ref.starts_with('-') {
        return Err(LibraryError::Parse {
            url: original_url.to_string(),
            source: serde::de::Error::custom(
                "git ref must not start with `-` (refused as potential argv-flag injection \
                 against `git fetch --upload-pack=...` and similar)",
            ),
        });
    }
    if git_ref
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\u{7f}')
    {
        return Err(LibraryError::Parse {
            url: original_url.to_string(),
            source: serde::de::Error::custom(
                "git ref must not contain whitespace or control bytes",
            ),
        });
    }

    // The repo half MUST still be an https:// URL after the @ trim
    // — unless the loopback-test bypass is active, in which case
    // file:// (pointing at a local repo) is also allowed.
    let loopback_allowed = std::env::var_os("JARVY_LIBRARY_ALLOW_INSECURE_GIT").is_some();
    let is_https = repo.starts_with("https://");
    let is_local_file = loopback_allowed && repo.starts_with("file://");
    if !is_https && !is_local_file {
        return Err(LibraryError::Parse {
            url: original_url.to_string(),
            source: serde::de::Error::custom("git source repo URL must be HTTPS"),
        });
    }

    Ok(SourceScheme::Git {
        repo: repo.to_string(),
        git_ref: git_ref.to_string(),
        subpath,
    })
}

/// Split the tail of a shorthand at the first `@` or `#` boundary so we
/// can validate the `@<ref>` pin before committing to a synthesized URL.
fn split_suffix(rest: &str) -> (&str, &str) {
    match rest.find(['@', '#']) {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_manifest_url() {
        let s = parse_source("https://cdn.example.com/manifest.json").unwrap();
        assert_eq!(
            s,
            SourceScheme::Manifest {
                url: "https://cdn.example.com/manifest.json".into()
            }
        );
    }

    #[test]
    fn parses_git_https_with_tag() {
        let s = parse_source("git+https://github.com/myorg/skills.git@v1.2.0").unwrap();
        assert_eq!(
            s,
            SourceScheme::Git {
                repo: "https://github.com/myorg/skills.git".into(),
                git_ref: "v1.2.0".into(),
                subpath: None,
            }
        );
    }

    #[test]
    fn parses_git_https_with_subpath() {
        let s = parse_source("git+https://github.com/myorg/skills.git@main#skills/").unwrap();
        assert_eq!(
            s,
            SourceScheme::Git {
                repo: "https://github.com/myorg/skills.git".into(),
                git_ref: "main".into(),
                subpath: Some("skills/".into()),
            }
        );
    }

    #[test]
    fn parses_github_shorthand() {
        let s = parse_source("github:myorg/skills@v1.2.0").unwrap();
        assert_eq!(
            s,
            SourceScheme::Git {
                repo: "https://github.com/myorg/skills.git".into(),
                git_ref: "v1.2.0".into(),
                subpath: None,
            }
        );
    }

    #[test]
    fn parses_github_shorthand_with_subpath() {
        let s = parse_source("github:myorg/skills@abc1234#skills/code-review").unwrap();
        assert_eq!(
            s,
            SourceScheme::Git {
                repo: "https://github.com/myorg/skills.git".into(),
                git_ref: "abc1234".into(),
                subpath: Some("skills/code-review".into()),
            }
        );
    }

    #[test]
    fn github_shorthand_preserves_existing_git_suffix() {
        let s = parse_source("github:myorg/skills.git@v1.0.0").unwrap();
        if let SourceScheme::Git { repo, .. } = s {
            // Should not produce skills.git.git.
            assert_eq!(repo, "https://github.com/myorg/skills.git");
        } else {
            panic!("expected Git variant");
        }
    }

    #[test]
    fn refuses_unpinned_git_url() {
        let err = parse_source("git+https://github.com/myorg/skills.git").unwrap_err();
        assert!(format!("{err}").contains("@<ref>"));
    }

    #[test]
    fn refuses_unpinned_github_shorthand() {
        let err = parse_source("github:myorg/skills").unwrap_err();
        assert!(format!("{err}").contains("@<ref>"));
    }

    /// P0 — argv-flag-injection refusal. `git fetch origin --upload-pack=cmd`
    /// runs `cmd` locally during transport setup. A ref starting with
    /// `-` is never legitimate; refuse at parse time.
    #[test]
    fn refuses_dash_prefixed_ref() {
        for hostile in [
            "git+https://github.com/myorg/skills.git@--upload-pack=curl",
            "git+https://github.com/myorg/skills.git@-X",
            "github:myorg/skills@-",
            "github:myorg/skills@--receive-pack=evil",
        ] {
            let err = parse_source(hostile).expect_err(hostile);
            let msg = format!("{err}");
            assert!(
                msg.contains("argv-flag injection") || msg.contains("must not start"),
                "expected argv-injection refusal for {hostile}, got {msg}"
            );
        }
    }

    /// Defense in depth — refuse whitespace / control bytes in git ref.
    #[test]
    fn refuses_whitespace_or_control_in_ref() {
        for hostile in [
            "github:myorg/skills@v1.0.0\n",
            "github:myorg/skills@v1 0",
            "github:myorg/skills@v1\u{0007}", // BEL
            "github:myorg/skills@v1\u{001b}[31m",
        ] {
            let err = parse_source(hostile).expect_err(hostile);
            let msg = format!("{err}");
            assert!(
                msg.contains("whitespace") || msg.contains("control"),
                "expected whitespace/control refusal for {hostile:?}, got {msg}"
            );
        }
    }

    #[test]
    fn refuses_subpath_with_dotdot() {
        let err =
            parse_source("git+https://github.com/myorg/skills.git@v1#../etc/passwd").unwrap_err();
        assert!(format!("{err}").contains(".."));
    }

    #[test]
    fn refuses_absolute_subpath() {
        let err =
            parse_source("git+https://github.com/myorg/skills.git@v1#/etc/passwd").unwrap_err();
        assert!(format!("{err}").contains("relative"));
    }

    #[test]
    fn refuses_unknown_scheme() {
        let err = parse_source("ftp://example.com/skills.json").unwrap_err();
        assert!(format!("{err}").contains("https://"));
    }

    #[test]
    fn refuses_plain_http() {
        let err = parse_source("http://example.com/skills.json").unwrap_err();
        assert!(format!("{err}").contains("https://"));
    }
}
