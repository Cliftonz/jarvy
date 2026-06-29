//! `[discover]` config block (PRD-044 phase 2).
//!
//! Lets users extend `jarvy discover` with their own detection rules
//! without forking the project. Today the only supported shape is a
//! path to a TOML file that deserializes into `Vec<DetectionRule>`:
//!
//! ```toml
//! [discover]
//! rules = ".jarvy/discovery-rules.toml"
//! ignore_dirs = ["vendor", "third_party"]
//! ```
//!
//! Custom rules are APPENDED to the built-in set — they can't remove
//! or override built-in rules. That posture is intentional: a user
//! tree shouldn't be able to silence a real ecosystem detection.

use super::rules::DetectionRule;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiscoverConfig {
    /// Path to a TOML file containing extra `DetectionRule` entries.
    /// Relative to the jarvy.toml project root.
    #[serde(default)]
    pub rules: Option<String>,

    /// Directory names to skip during detection (e.g. `vendor`,
    /// `node_modules`). Today only the `*.ext` glob path consults
    /// this list — the rest of the scanner walks only the project
    /// root and ignores subdirs by design.
    #[serde(default)]
    pub ignore_dirs: Vec<String>,
}

/// Shape of the custom rules file. Just a wrapper around a
/// `Vec<DetectionRule>` so the TOML file can carry top-level
/// section comments / docs alongside the array.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CustomRulesFile {
    #[serde(default)]
    pub rules: Vec<DetectionRule>,
}

/// Load + merge the rule set. The built-in `default_rules()` always
/// applies; `cfg.rules` (when present) appends to it. Returns the
/// combined slice and a count of how many came from the custom file
/// (zero when no file was loaded).
///
/// Errors are surfaced as a tuple of `(default_rules, advisory)` so
/// the caller can decide whether to continue with built-ins only.
/// Failing closed on a parse error would lock users out of `jarvy
/// discover` whenever their custom rules file regresses.
pub fn load_effective_rules(
    project_dir: &Path,
    cfg: Option<&DiscoverConfig>,
) -> (Vec<DetectionRule>, Vec<String>) {
    let mut combined: Vec<DetectionRule> = super::default_rules().to_vec();
    let mut advisories: Vec<String> = Vec::new();

    let Some(cfg) = cfg else {
        return (combined, advisories);
    };
    let Some(rules_path) = cfg.rules.as_ref() else {
        return (combined, advisories);
    };

    let path = project_dir.join(rules_path);
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            advisories.push(format!(
                "[discover] rules file `{}` could not be read: {e} (built-in rules only)",
                path.display()
            ));
            return (combined, advisories);
        }
    };

    match toml::from_str::<CustomRulesFile>(&content) {
        Ok(parsed) => {
            combined.extend(parsed.rules);
        }
        Err(e) => {
            advisories.push(format!(
                "[discover] rules file `{}` failed to parse: {e} (built-in rules only)",
                path.display()
            ));
        }
    }
    (combined, advisories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn no_config_yields_only_default_rules() {
        let tmp = tempdir().unwrap();
        let (rules, adv) = load_effective_rules(tmp.path(), None);
        assert!(!rules.is_empty(), "default_rules must be non-empty");
        assert_eq!(adv, Vec::<String>::new());
    }

    #[test]
    fn custom_rules_file_appends_entries() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("custom.toml"),
            r#"
[[rules]]
name = "custom-tool"
category = "dev"

[[rules.detect]]
file = ".custom-marker"
"#,
        )
        .unwrap();
        let cfg = DiscoverConfig {
            rules: Some("custom.toml".into()),
            ignore_dirs: vec![],
        };
        let (rules, adv) = load_effective_rules(tmp.path(), Some(&cfg));
        assert_eq!(adv, Vec::<String>::new());
        assert!(rules.iter().any(|r| r.name == "custom-tool"));
        // Built-ins are still present.
        assert!(rules.iter().any(|r| r.name == "rust"));
    }

    #[test]
    fn missing_file_emits_advisory_not_error() {
        let tmp = tempdir().unwrap();
        let cfg = DiscoverConfig {
            rules: Some("nope.toml".into()),
            ignore_dirs: vec![],
        };
        let (rules, adv) = load_effective_rules(tmp.path(), Some(&cfg));
        assert!(!rules.is_empty());
        assert_eq!(adv.len(), 1);
        assert!(adv[0].contains("could not be read"));
    }

    #[test]
    fn malformed_file_emits_advisory_not_error() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("bad.toml"), "not [ valid toml @").unwrap();
        let cfg = DiscoverConfig {
            rules: Some("bad.toml".into()),
            ignore_dirs: vec![],
        };
        let (_rules, adv) = load_effective_rules(tmp.path(), Some(&cfg));
        assert_eq!(adv.len(), 1);
        assert!(adv[0].contains("failed to parse"));
    }
}
