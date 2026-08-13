//! Drift detection configuration types

use serde::{Deserialize, Serialize};

/// Drift detection configuration section in jarvy.toml
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DriftConfig {
    /// Enable drift detection
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Check for drift on every jarvy command
    #[serde(default)]
    pub check_on_run: bool,

    /// Files to track for changes
    #[serde(default)]
    pub track_files: Vec<String>,

    /// Version matching policy
    #[serde(default)]
    pub version_policy: VersionPolicy,

    /// Tools to ignore during drift detection
    #[serde(default)]
    pub ignore_tools: Vec<String>,

    /// Allow upgrades (only flag downgrades as drift)
    #[serde(default)]
    pub allow_upgrades: bool,

    /// Remote-config trust gate. When the enclosing `jarvy.toml` was
    /// fetched via `--from <url>` (ConfigOrigin::Remote), `track_files`
    /// is refused unless `allow_remote = true`. Without this a hostile
    /// remote could set `track_files = ["/etc/shadow", ...]` and turn
    /// baseline capture into a SHA-256 file-existence oracle. Mirrors
    /// the `[packages]/[git_hooks]/[maintenance] allow_remote` pattern.
    #[serde(default)]
    pub allow_remote: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_on_run: false,
            track_files: Vec::new(),
            version_policy: VersionPolicy::Minor,
            ignore_tools: Vec::new(),
            allow_upgrades: false,
            allow_remote: false,
        }
    }
}

/// True if a `track_files` entry is safe to hash — rejects absolute
/// paths (Path::join discards the base for absolute rhs, so an entry
/// like `"/etc/shadow"` resolves outside project_dir) and any entry
/// containing a `..` component (naïve traversal). Callers pair this
/// with the canonicalize-based traversal check in
/// `state::path_is_within_project_dir` for defense in depth.
///
/// The refusal is deliberately cross-platform: a hostile Linux-authored
/// config with `"/etc/shadow"` MUST be refused on Windows too, where
/// `Path::is_absolute` returns false for the missing drive letter.
/// Same shape in reverse for `"C:\..."` on Linux. We check the string
/// shape ourselves so a Linux-shape absolute doesn't sneak past the
/// per-platform `is_absolute` gate.
pub fn track_file_is_safe(entry: &str) -> bool {
    let p = std::path::Path::new(entry);
    if p.is_absolute() {
        return false;
    }
    // Cross-platform absolute-shape refusal.
    let bytes = entry.as_bytes();
    match bytes.first() {
        Some(b'/') | Some(b'\\') => return false,
        _ => {}
    }
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return false;
    }
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    true
}

/// Version matching policy for drift detection
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VersionPolicy {
    /// Only major version must match (1.x.x)
    Major,
    /// Major and minor must match (1.2.x)
    #[default]
    Minor,
    /// Major, minor, and patch must match (1.2.3)
    Patch,
    /// Exact version required (including pre-release, build metadata)
    Exact,
}

impl VersionPolicy {
    /// Check if two versions match according to this policy
    pub fn versions_match(&self, expected: &str, actual: &str) -> bool {
        match self {
            VersionPolicy::Exact => expected == actual,
            VersionPolicy::Patch | VersionPolicy::Minor | VersionPolicy::Major => {
                let exp = semver::Version::parse(expected).ok();
                let act = semver::Version::parse(actual).ok();

                match (exp, act) {
                    (Some(e), Some(a)) => match self {
                        VersionPolicy::Patch => {
                            e.major == a.major && e.minor == a.minor && e.patch == a.patch
                        }
                        VersionPolicy::Minor => e.major == a.major && e.minor == a.minor,
                        VersionPolicy::Major => e.major == a.major,
                        VersionPolicy::Exact => unreachable!(),
                    },
                    // Fallback to string comparison if not valid semver
                    _ => expected == actual,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_policy_exact() {
        let policy = VersionPolicy::Exact;
        assert!(policy.versions_match("1.2.3", "1.2.3"));
        assert!(!policy.versions_match("1.2.3", "1.2.4"));
        assert!(!policy.versions_match("1.2.3", "1.2.3-beta"));
    }

    #[test]
    fn test_version_policy_patch() {
        let policy = VersionPolicy::Patch;
        assert!(policy.versions_match("1.2.3", "1.2.3"));
        assert!(!policy.versions_match("1.2.3", "1.2.4"));
        assert!(!policy.versions_match("1.2.3", "1.3.3"));
    }

    #[test]
    fn test_version_policy_minor() {
        let policy = VersionPolicy::Minor;
        assert!(policy.versions_match("1.2.3", "1.2.3"));
        assert!(policy.versions_match("1.2.3", "1.2.99"));
        assert!(!policy.versions_match("1.2.3", "1.3.0"));
        assert!(!policy.versions_match("1.2.3", "2.2.3"));
    }

    #[test]
    fn test_version_policy_major() {
        let policy = VersionPolicy::Major;
        assert!(policy.versions_match("1.2.3", "1.2.3"));
        assert!(policy.versions_match("1.2.3", "1.99.99"));
        assert!(!policy.versions_match("1.2.3", "2.0.0"));
    }

    #[test]
    fn test_version_policy_non_semver() {
        // Non-semver versions fall back to exact string comparison
        let policy = VersionPolicy::Minor;
        assert!(policy.versions_match("abc123", "abc123"));
        assert!(!policy.versions_match("abc123", "abc124"));
    }

    /// Regression pin for Codex P1 / parallel-review QA F2: the
    /// spec-vs-concrete arm (one semver, one not) was previously
    /// untested. Historically the setup-time write path stored the
    /// config constraint (`"latest"`, `"^20"`) into ToolState.version;
    /// check-time then compared that string against the probed
    /// concrete version, and this fallback returned false for every
    /// tool — resulting in the "22 tools drift on every check" loop.
    ///
    /// The fix moved the concrete version onto ToolState.installed_version
    /// so this arm is no longer reachable in practice; the test pins
    /// the semantic (`(constraint, concrete) → false`) so a future
    /// refactor that undoes the field split fails loudly here first.
    #[test]
    fn version_policy_constraint_vs_concrete_returns_false() {
        for policy in [
            VersionPolicy::Major,
            VersionPolicy::Minor,
            VersionPolicy::Patch,
            VersionPolicy::Exact,
        ] {
            for (constraint, concrete) in [
                ("latest", "20.10.0"),
                ("^20", "20.10.0"),
                (">=1.75", "1.75.0"),
                ("20.10.0", "latest"),
            ] {
                assert!(
                    !policy.versions_match(constraint, concrete),
                    "{policy:?} matched constraint {constraint:?} against concrete {concrete:?} — \
                     did the string-fallback semantics change?"
                );
            }
        }
    }

    #[test]
    fn test_drift_config_defaults() {
        let config = DriftConfig::default();
        assert!(config.enabled);
        assert!(!config.check_on_run);
        assert!(config.track_files.is_empty());
        assert_eq!(config.version_policy, VersionPolicy::Minor);
        assert!(config.ignore_tools.is_empty());
        assert!(!config.allow_upgrades);
        assert!(!config.allow_remote);
    }

    #[test]
    fn track_file_safety_rejects_absolute_and_traversal() {
        // Safe: project-relative descendants.
        assert!(track_file_is_safe("package.json"));
        assert!(track_file_is_safe(".vscode/settings.json"));
        assert!(track_file_is_safe("some/nested/file"));

        // Unsafe: absolute paths (Path::join(rhs) drops project_dir
        // when rhs is absolute — this is the primitive CVE would use).
        assert!(!track_file_is_safe("/etc/shadow"));
        assert!(!track_file_is_safe("/Users/victim/.ssh/id_rsa"));

        // Unsafe: any parent-dir component.
        assert!(!track_file_is_safe("../etc/passwd"));
        assert!(!track_file_is_safe("../../secret"));
        assert!(!track_file_is_safe("nested/../../escape"));
    }

    /// A Linux-authored hostile config MUST be refused on Windows too,
    /// even though `Path::is_absolute("/etc/shadow")` returns false on
    /// Windows (no drive letter). Symmetric for Windows-authored
    /// `C:\...` on Linux. Regression: a Windows CI runner silently
    /// treated `/etc/shadow` as a project-relative path because the
    /// `is_absolute` gate is per-platform. See rc.4 CI failure.
    #[test]
    fn track_file_safety_refuses_foreign_platform_absolutes() {
        // Unix-shape absolutes (must fail on ALL platforms, including
        // Windows where Path::is_absolute returns false for these).
        for entry in [
            "/etc/shadow",
            "/tmp/leak",
            "/root/.ssh/id_rsa",
            "/usr/local/bin/tool",
            "/",
        ] {
            assert!(
                !track_file_is_safe(entry),
                "unix-shape absolute must be refused cross-platform: {entry:?}"
            );
        }

        // Windows-shape absolutes (must fail on Linux where
        // Path::is_absolute returns false for these).
        for entry in [
            "C:\\Windows\\System32\\config\\SAM",
            "D:/data/leak.txt",
            "c:\\lowercase\\drive",
            "Z:/other/drive",
        ] {
            assert!(
                !track_file_is_safe(entry),
                "windows-shape absolute must be refused cross-platform: {entry:?}"
            );
        }

        // UNC / backslash-prefix paths.
        for entry in ["\\Windows\\System32", "\\\\server\\share\\secret"] {
            assert!(
                !track_file_is_safe(entry),
                "backslash-prefix path must be refused cross-platform: {entry:?}"
            );
        }
    }

    /// Guard against over-broad refusal: single-letter names, colon in
    /// the middle (not the drive position), backslashes inside a
    /// relative path — these are legitimate on POSIX and must stay safe.
    #[test]
    fn track_file_safety_does_not_over_refuse_legit_relatives() {
        // Single-char filename — no colon in position 1, so not a drive.
        assert!(track_file_is_safe("a"));
        assert!(track_file_is_safe("z"));
        // Colon later in the name is a valid POSIX filename character.
        assert!(track_file_is_safe("weird:file.txt"));
        // Non-alpha before colon — not a drive letter.
        assert!(track_file_is_safe("1:leading-digit.txt"));
        // Dotfile / hidden — always safe if relative.
        assert!(track_file_is_safe(".env"));
        assert!(track_file_is_safe(".config/settings"));
    }

    #[test]
    fn test_drift_config_parsing() {
        let toml_str = r#"
enabled = true
check_on_run = false
track_files = [".vscode/settings.json", "package.json"]
version_policy = "minor"
ignore_tools = ["vim", "neovim"]
allow_upgrades = true
"#;
        let config: DriftConfig = toml::from_str(toml_str).expect("Failed to parse config");

        assert!(config.enabled);
        assert!(!config.check_on_run);
        assert_eq!(config.track_files.len(), 2);
        assert_eq!(config.version_policy, VersionPolicy::Minor);
        assert_eq!(config.ignore_tools, vec!["vim", "neovim"]);
        assert!(config.allow_upgrades);
    }
}
