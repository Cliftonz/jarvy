//! Skill fetch + install pipeline.
//!
//! For each `(skill_name, version)` in `SkillsConfig::install`:
//!
//! 1. Resolve via `library_registry::resolve_skill(name)` — returns a
//!    `LibrarySkillItem` with `skill_md_url` + `skill_md_sha256`.
//! 2. Refuse if the requested `version` doesn't match the library
//!    item's `version` (no version drift surprises).
//! 3. Fetch `SKILL.md` over HTTPS (bounded read).
//! 4. sha256-verify against the manifest entry.
//! 5. Write to every target agent's `skills/<name>/SKILL.md`.
//! 6. Drop a `.jarvy-skill.json` sidecar so subsequent runs can detect
//!    drift and `jarvy skills status` can report what's installed.

use super::agents::SkillAgent;
use super::config::SkillEntry;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillError {
    #[error(
        "skill `{0}` not found in any configured library_source. \
         Verify the source URL resolves to a manifest.json OR a git repo containing \
         `SKILL.md` files with YAML frontmatter (name, version required). \
         See https://jarvy.dev/skills/#authoring"
    )]
    NotInLibrary(String),

    #[error(
        "skill `{name}` version mismatch: jarvy.toml requests `{requested}`, \
         library_source advertises `{advertised}`"
    )]
    VersionMismatch {
        name: String,
        requested: String,
        advertised: String,
    },

    #[error("fetch failed for {url}: {source}")]
    Fetch {
        url: String,
        #[source]
        source: crate::library_registry::fetch::FetchError,
    },

    #[error(
        "sha256 mismatch for `{name}`: manifest declares `{expected}`, \
         fetched body computes `{actual}`"
    )]
    ShaMismatch {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("no agents installed; nothing to install to")]
    NoAgents,
}

impl SkillError {
    #[allow(dead_code)] // Public telemetry-discriminant API
    pub fn kind(&self) -> &'static str {
        match self {
            SkillError::NotInLibrary(_) => "not_in_library",
            SkillError::VersionMismatch { .. } => "version_mismatch",
            SkillError::Fetch { .. } => "fetch",
            SkillError::ShaMismatch { .. } => "sha_mismatch",
            SkillError::Io(_) => "io",
            SkillError::NoAgents => "no_agents",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallResult {
    #[allow(dead_code)] // Surfaced via Debug + structured callers
    pub name: String,
    pub version: String,
    pub agents: Vec<SkillAgent>,
    pub skipped_agents: Vec<(SkillAgent, &'static str)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillStatus {
    Installed {
        version: String,
    },
    Missing,
    Drift {
        installed: String,
        requested: String,
    },
}

/// Install a single skill across the given agents. Caller is
/// responsible for having called `library_registry::sync(...)` for
/// every relevant source first.
pub fn install_skill(
    name: &str,
    entry: &SkillEntry,
    target_agents: &[SkillAgent],
) -> Result<InstallResult, SkillError> {
    if target_agents.is_empty() {
        return Err(SkillError::NoAgents);
    }

    let item = crate::library_registry::resolve_skill(name)
        .ok_or_else(|| SkillError::NotInLibrary(name.to_string()))?;

    let requested = entry.version();
    if requested != "latest" && requested != item.version {
        return Err(SkillError::VersionMismatch {
            name: name.to_string(),
            requested: requested.to_string(),
            advertised: item.version.clone(),
        });
    }

    // Per-entry agent narrowing.
    let narrow: HashSet<&str> = entry.agents().iter().map(String::as_str).collect();
    let supported: HashSet<&str> = item.supported_agents.iter().map(String::as_str).collect();

    let mut installed_agents = Vec::new();
    let mut skipped = Vec::new();

    let body = fetch_skill_md(&item)?;

    for agent in target_agents {
        if !narrow.is_empty() && !narrow.contains(agent.slug()) {
            skipped.push((*agent, "not in entry agents narrowing"));
            continue;
        }
        if !supported.is_empty() && !supported.contains(agent.slug()) {
            skipped.push((*agent, "not in library supported_agents"));
            continue;
        }

        let Some(skills_root) = agent.skills_dir() else {
            skipped.push((*agent, "home directory lookup failed"));
            continue;
        };
        let skill_dir = skills_root.join(name);
        std::fs::create_dir_all(&skill_dir)?;
        let skill_md_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_md_path, &body)?;
        write_sidecar(&skill_dir, name, &item.version, &item.skill_md_sha256)?;
        installed_agents.push(*agent);
    }

    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "skills.installed",
            skill = %name,
            version = %item.version,
            agent_count = installed_agents.len(),
            skipped_count = skipped.len(),
        );
    }

    Ok(InstallResult {
        name: name.to_string(),
        version: item.version.clone(),
        agents: installed_agents,
        skipped_agents: skipped,
    })
}

/// Probe whether `skill_name` is installed for `agent`. Returns the
/// version recorded in the sidecar JSON. Used by `jarvy skills status`.
pub fn skill_status(skill_name: &str, requested_version: &str, agent: SkillAgent) -> SkillStatus {
    let Some(skills_root) = agent.skills_dir() else {
        return SkillStatus::Missing;
    };
    let sidecar = skills_root.join(skill_name).join(".jarvy-skill.json");
    if !sidecar.exists() {
        return SkillStatus::Missing;
    }
    let Ok(bytes) = std::fs::read(&sidecar) else {
        return SkillStatus::Missing;
    };
    let Ok(meta): Result<SidecarMeta, _> = serde_json::from_slice(&bytes) else {
        return SkillStatus::Missing;
    };
    if requested_version == "latest" || meta.version == requested_version {
        SkillStatus::Installed {
            version: meta.version,
        }
    } else {
        SkillStatus::Drift {
            installed: meta.version,
            requested: requested_version.to_string(),
        }
    }
}

fn fetch_skill_md(item: &crate::library_registry::LibrarySkillItem) -> Result<Vec<u8>, SkillError> {
    // PRD-055: git-fetched libraries synthesize `skill_md_url` as a
    // `file://` URL pointing into the local clone cache. Branch here
    // so the existing HTTPS fetcher (which would refuse non-HTTPS)
    // stays clean.
    let body = if item.skill_md_url.starts_with("file://") {
        crate::library_registry::git_fetch::read_file_url(&item.skill_md_url).map_err(|e| {
            SkillError::Fetch {
                url: crate::network::redact_credentials(&item.skill_md_url).into_owned(),
                // file:// reads surface as LibraryError::Io; map into
                // the FetchError envelope by re-wrapping the io error.
                source: crate::library_registry::fetch::FetchError::Read {
                    url: crate::network::redact_credentials(&item.skill_md_url).into_owned(),
                    source: match e {
                        crate::library_registry::LibraryError::Io(io) => io,
                        other => std::io::Error::other(format!("{other}")),
                    },
                },
            }
        })?
    } else {
        crate::library_registry::fetch::fetch_bounded(
            &item.skill_md_url,
            crate::library_registry::fetch::MAX_ITEM_BYTES,
        )
        .map_err(|e| SkillError::Fetch {
            url: crate::network::redact_credentials(&item.skill_md_url).into_owned(),
            source: e,
        })?
    };

    let actual = sha256_hex(&body);
    if !actual.eq_ignore_ascii_case(&item.skill_md_sha256) {
        return Err(SkillError::ShaMismatch {
            name: item.name.clone(),
            expected: item.skill_md_sha256.clone(),
            actual,
        });
    }
    Ok(body)
}

fn write_sidecar(skill_dir: &Path, name: &str, version: &str, sha256: &str) -> std::io::Result<()> {
    let meta = SidecarMeta {
        skill: name.to_string(),
        version: version.to_string(),
        skill_md_sha256: sha256.to_string(),
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    let bytes = serde_json::to_vec_pretty(&meta).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("sidecar: {e}"))
    })?;
    std::fs::write(skill_dir.join(".jarvy-skill.json"), bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SidecarMeta {
    skill: String,
    version: String,
    skill_md_sha256: String,
    installed_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_stable() {
        assert_eq!(sha256_hex(b"hello"), sha256_hex(b"hello"));
        assert_ne!(sha256_hex(b"hello"), sha256_hex(b"world"));
    }

    #[test]
    fn skill_status_missing_for_unknown_skill() {
        // No JARVY_HOME → home_dir() may resolve elsewhere, but the
        // sidecar path is virtually guaranteed to not exist.
        let status = skill_status(
            "definitely-not-installed-12345",
            "latest",
            SkillAgent::ClaudeCode,
        );
        assert_eq!(status, SkillStatus::Missing);
    }

    // =================================================================
    // Review item 11 (P0) — fetch_skill_md sha-mismatch path.
    //
    // The whole point of fetching by sha256 is tamper detection. The
    // E2E suite uses the synthesizer-computed sha (always matches) —
    // a refactor that drops the verification line ships green. These
    // tests pin the contract directly against a fixture skill body in
    // the library cache root, using a publisher-supplied sha that may
    // or may not match.
    // =================================================================

    use crate::library_registry::manifest::LibrarySkillItem;
    use serial_test::serial;
    use tempfile::tempdir;

    fn seed_library_cache_skill_md(content: &[u8]) -> (tempfile::TempDir, String) {
        let home = tempdir().unwrap();
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", home.path());
        }
        let cache_root = crate::library_registry::cache::cache_root().unwrap();
        let skill_path = cache_root.join("test-skill.md");
        std::fs::write(&skill_path, content).unwrap();
        let url = format!("file://{}", skill_path.canonicalize().unwrap().display());
        (home, url)
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn fetch_skill_md_refuses_sha_mismatch() {
        let (_home_guard, url) = seed_library_cache_skill_md(b"actual body bytes");
        let item = LibrarySkillItem {
            name: "test-skill".into(),
            version: "1.0.0".into(),
            description: String::new(),
            skill_md_url: url,
            skill_md_sha256: "deadbeef".into(), // deliberate wrong sha
            companion_files: Vec::new(),
            supported_agents: Vec::new(),
        };
        let err = fetch_skill_md(&item).expect_err("wrong sha must refuse");
        match err {
            SkillError::ShaMismatch {
                name,
                expected,
                actual,
            } => {
                assert_eq!(name, "test-skill");
                assert_eq!(expected, "deadbeef");
                assert_ne!(actual, "deadbeef", "actual must be computed from body");
            }
            other => panic!("expected ShaMismatch, got {other:?}"),
        }
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    #[test]
    #[serial(jarvy_home_env)]
    fn fetch_skill_md_accepts_correct_sha_case_insensitive() {
        let body = b"verified body";
        let (_home_guard, url) = seed_library_cache_skill_md(body);
        // Compute the expected sha, then UPPERCASE it to verify the
        // case-insensitive comparison is honored.
        let expected = sha256_hex(body).to_uppercase();
        let item = LibrarySkillItem {
            name: "test-skill".into(),
            version: "1.0.0".into(),
            description: String::new(),
            skill_md_url: url,
            skill_md_sha256: expected,
            companion_files: Vec::new(),
            supported_agents: Vec::new(),
        };
        let bytes = fetch_skill_md(&item).expect("matching uppercase sha must accept");
        assert_eq!(bytes, body);
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    #[test]
    fn install_skill_no_agents_returns_no_agents() {
        let entry = crate::skills::config::SkillEntry::Version("1.0.0".to_string());
        let err = install_skill("any-skill", &entry, &[]).expect_err("empty agents must error");
        match err {
            SkillError::NoAgents => {}
            other => panic!("expected NoAgents, got {other:?}"),
        }
    }
}
