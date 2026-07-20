//! `[skills]` configuration schema.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ai_hooks::ConfigOrigin;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkillsConfig {
    /// Auto-install skills during `jarvy setup` (default true). When
    /// false, the block is parsed but no installation happens until
    /// `jarvy skills install` runs manually.
    #[serde(default = "default_true")]
    pub auto_install: bool,

    /// Which agents to install skills for. Empty = auto-detect every
    /// AI agent installed on disk. Use `["claude-code", "cursor"]` to
    /// narrow.
    #[serde(default)]
    pub agents: Vec<String>,

    /// Skill entries by name. Each value is either a bare version
    /// string or a detailed `{ version = "...", agents = [...] }`
    /// table for per-skill agent narrowing.
    #[serde(default)]
    pub install: HashMap<String, SkillEntry>,

    /// Library manifests to consult for skill definitions (PRD-054).
    /// Local-origin only — remote-fetched configs are refused at
    /// install time by `library_registry::check_origin`.
    #[serde(default)]
    pub library_sources: Vec<crate::library_registry::LibrarySource>,

    /// Origin tag set by the config loader; not serialized.
    #[serde(skip)]
    pub origin: ConfigOrigin,
}

impl SkillsConfig {
    #[allow(dead_code)] // Public API; reserved for setup-phase short-circuit
    pub fn is_empty(&self) -> bool {
        self.install.is_empty()
    }
}

impl crate::ai_hooks::HasOrigin for SkillsConfig {
    fn set_origin(&mut self, origin: ConfigOrigin) {
        self.origin = origin;
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SkillEntry {
    /// `myorg-code-review = "2.1.0"` — bare version string. `"latest"`
    /// pulls whatever the library_source currently advertises.
    Version(String),
    /// Inline table with optional per-skill agent narrowing.
    Detailed {
        version: String,
        #[serde(default)]
        agents: Vec<String>,
    },
}

impl SkillEntry {
    pub fn version(&self) -> &str {
        match self {
            SkillEntry::Version(v) => v,
            SkillEntry::Detailed { version, .. } => version,
        }
    }

    /// Per-skill agent narrowing. Empty list = "all configured agents".
    pub fn agents(&self) -> &[String] {
        match self {
            SkillEntry::Version(_) => &[],
            SkillEntry::Detailed { agents, .. } => agents,
        }
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_via_serde() {
        let cfg: SkillsConfig = toml::from_str("").unwrap();
        assert!(cfg.auto_install);
        assert!(cfg.agents.is_empty());
        assert!(cfg.install.is_empty());
        assert!(cfg.library_sources.is_empty());
    }

    #[test]
    fn parses_bare_version() {
        let toml_str = r#"
[install]
myorg-code-review = "2.1.0"
"#;
        let cfg: SkillsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.install.len(), 1);
        match &cfg.install["myorg-code-review"] {
            SkillEntry::Version(v) => assert_eq!(v, "2.1.0"),
            other => panic!("expected Version, got {other:?}"),
        }
    }

    #[test]
    fn parses_detailed_with_agents() {
        let toml_str = r#"
[install]
myorg-code-review = { version = "2.1.0", agents = ["claude-code"] }
"#;
        let cfg: SkillsConfig = toml::from_str(toml_str).unwrap();
        match &cfg.install["myorg-code-review"] {
            SkillEntry::Detailed { version, agents } => {
                assert_eq!(version, "2.1.0");
                assert_eq!(agents, &["claude-code"]);
            }
            other => panic!("expected Detailed, got {other:?}"),
        }
    }

    #[test]
    fn parses_library_sources() {
        let toml_str = r#"
[[library_sources]]
url = "https://cdn.example.com/manifest.json"
"#;
        let cfg: SkillsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.library_sources.len(), 1);
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let toml_str = "garbage = true";
        let err = toml::from_str::<SkillsConfig>(toml_str).unwrap_err();
        assert!(format!("{err}").contains("garbage"));
    }
}
