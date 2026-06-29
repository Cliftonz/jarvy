//! Detection rule engine for `jarvy discover`.
//!
//! Each `DetectionRule` declares marker files / directories that indicate
//! a technology is in use, where to extract its version, and what
//! companion tools to recommend. The bundled `default_rules()` set covers
//! the main ecosystems jarvy supports today (rust, node, python, go,
//! docker, kubectl, terraform, pre-commit). Adding a new ecosystem is one
//! entry in the array — no other code changes needed.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::scanner::find_first_match;
use super::version::extract_version;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRule {
    pub name: String,
    pub detect: Vec<DetectionPattern>,
    #[serde(default)]
    pub version_from: Option<VersionSource>,
    #[serde(default)]
    pub suggests: Vec<String>,
    pub category: ToolCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DetectionPattern {
    File { file: String },
    Dir { dir: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSource {
    pub file: String,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    Runtime,
    Build,
    Dev,
    Ops,
}

/// One technology successfully detected in the project tree. The
/// `source` field is human-readable (e.g. "Cargo.toml") and surfaces in
/// the `--format pretty` output so users can see why a tool was
/// suggested.
#[derive(Debug, Clone, Serialize)]
pub struct Detection {
    pub tool: String,
    pub version: Option<String>,
    pub source: String,
    pub suggests: Vec<String>,
    pub category: ToolCategory,
}

/// Walk every rule against `project_dir` and return one `Detection` per
/// matched rule. Stable iteration order matches `rules` for
/// deterministic output.
pub fn run(project_dir: &Path, rules: &[DetectionRule]) -> Vec<Detection> {
    let mut out = Vec::new();
    for rule in rules {
        if let Some(matched_source) = rule_match_source(project_dir, rule) {
            let version = rule
                .version_from
                .as_ref()
                .and_then(|vs| extract_version(project_dir, vs));
            out.push(Detection {
                tool: rule.name.clone(),
                version,
                source: matched_source,
                suggests: rule.suggests.clone(),
                category: rule.category,
            });
        }
    }
    out
}

/// First matching pattern wins; we return its source string so the
/// suggestion explainer can cite a real file ("detected from Cargo.toml").
///
/// The source string flows into the rendered `# detected from ...`
/// comment in `discover/generator.rs`. For pattern-supplied filenames
/// (`Cargo.toml`, `package.json`, ...) that's a trusted rule-author
/// literal. For `*.ext` glob matches the on-disk filename is
/// attacker-controllable (POSIX filenames may contain newlines, `"`,
/// and control bytes). We strict-allowlist the matched filename to
/// printable ASCII without quotes/backslashes so a hostile filename
/// like `x.tf\n[packages]\nallow_remote = true\n# .tf` can't inject
/// a TOML section through the rendered comment (review item P0 #2).
fn rule_match_source(project_dir: &Path, rule: &DetectionRule) -> Option<String> {
    for pattern in &rule.detect {
        match pattern {
            DetectionPattern::File { file } => {
                if let Some(p) = find_first_match(project_dir, file) {
                    let name = p.file_name()?.to_string_lossy().into_owned();
                    if let Some(safe) = sanitize_source(&name) {
                        return Some(safe);
                    }
                    // Hostile filename — skip this pattern but still try
                    // siblings so a `*.tf` rule isn't defeated by one
                    // poisoned filename.
                    continue;
                }
            }
            DetectionPattern::Dir { dir } => {
                if project_dir.join(dir).is_dir() {
                    if let Some(safe) = sanitize_source(dir) {
                        return Some(safe);
                    }
                }
            }
        }
    }
    None
}

/// Accept only ASCII-graphic + space — every other byte (newline,
/// CR, NUL, ESC, DEL, `"`, `\`) is refused. Returning `None` on a
/// hostile filename means the rule doesn't fire at all rather than
/// allowing partial / sanitized attribution.
fn sanitize_source(name: &str) -> Option<String> {
    if name.is_empty() || name.len() > 255 {
        return None;
    }
    if !name
        .chars()
        .all(|c| (c.is_ascii_graphic() && c != '"' && c != '\\') || c == ' ')
    {
        return None;
    }
    Some(name.to_string())
}

/// Built-in detection rules covering the ecosystems jarvy ships handlers
/// for today. Names match the canonical jarvy tool name (lowercase,
/// dash-separated) so `analyze()` can validate suggestions against
/// `tools::registry::registered_tool_names()` without aliasing logic.
///
/// Cached behind a `OnceLock` (review item P2 #22) — the ~50 String
/// allocations are paid once per process, not per `analyze()` call.
pub fn default_rules() -> &'static [DetectionRule] {
    use std::sync::OnceLock;
    static RULES: OnceLock<Vec<DetectionRule>> = OnceLock::new();
    RULES.get_or_init(build_default_rules)
}

fn build_default_rules() -> Vec<DetectionRule> {
    vec![
        DetectionRule {
            name: "rust".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Cargo.toml".into(),
                },
                DetectionPattern::File {
                    file: "Cargo.lock".into(),
                },
                DetectionPattern::File {
                    file: "rust-toolchain.toml".into(),
                },
                DetectionPattern::File {
                    file: "rust-toolchain".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: "rust-toolchain.toml".into(),
                pattern: Some(r#"channel\s*=\s*"([^"]+)""#.into()),
            }),
            suggests: vec!["cargo-watch".into(), "cargo-nextest".into()],
            category: ToolCategory::Runtime,
        },
        DetectionRule {
            name: "node".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "package.json".into(),
                },
                DetectionPattern::File {
                    file: "package-lock.json".into(),
                },
                DetectionPattern::File {
                    file: "yarn.lock".into(),
                },
                DetectionPattern::File {
                    file: "pnpm-lock.yaml".into(),
                },
                DetectionPattern::File {
                    file: ".nvmrc".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: ".nvmrc".into(),
                pattern: Some(r"v?(\d+(?:\.\d+(?:\.\d+)?)?)".into()),
            }),
            suggests: vec!["pnpm".into(), "yarn".into()],
            category: ToolCategory::Runtime,
        },
        DetectionRule {
            name: "python".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "pyproject.toml".into(),
                },
                DetectionPattern::File {
                    file: "requirements.txt".into(),
                },
                DetectionPattern::File {
                    file: "Pipfile".into(),
                },
                DetectionPattern::File {
                    file: "setup.py".into(),
                },
                DetectionPattern::File {
                    file: ".python-version".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: ".python-version".into(),
                pattern: None,
            }),
            suggests: vec!["uv".into(), "poetry".into(), "pipx".into()],
            category: ToolCategory::Runtime,
        },
        DetectionRule {
            name: "go".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "go.mod".into(),
                },
                DetectionPattern::File {
                    file: "go.sum".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: "go.mod".into(),
                pattern: Some(r"^go\s+(\d+\.\d+(?:\.\d+)?)".into()),
            }),
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        DetectionRule {
            name: "ruby".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Gemfile".into(),
                },
                DetectionPattern::File {
                    file: "Gemfile.lock".into(),
                },
                DetectionPattern::File {
                    file: ".ruby-version".into(),
                },
            ],
            version_from: Some(VersionSource {
                file: ".ruby-version".into(),
                pattern: None,
            }),
            suggests: vec![],
            category: ToolCategory::Runtime,
        },
        DetectionRule {
            name: "docker".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Dockerfile".into(),
                },
                DetectionPattern::File {
                    file: "docker-compose.yml".into(),
                },
                DetectionPattern::File {
                    file: "docker-compose.yaml".into(),
                },
                DetectionPattern::File {
                    file: "compose.yml".into(),
                },
                DetectionPattern::File {
                    file: "compose.yaml".into(),
                },
            ],
            version_from: None,
            suggests: vec!["docker-compose".into(), "lazydocker".into()],
            category: ToolCategory::Ops,
        },
        DetectionRule {
            name: "kubectl".into(),
            detect: vec![
                DetectionPattern::Dir { dir: "k8s".into() },
                DetectionPattern::Dir {
                    dir: "kubernetes".into(),
                },
                DetectionPattern::Dir {
                    dir: "manifests".into(),
                },
            ],
            version_from: None,
            suggests: vec!["helm".into(), "kustomize".into(), "k9s".into()],
            category: ToolCategory::Ops,
        },
        DetectionRule {
            name: "helm".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Chart.yaml".into(),
                },
                DetectionPattern::Dir {
                    dir: "charts".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Ops,
        },
        DetectionRule {
            name: "terraform".into(),
            detect: vec![
                DetectionPattern::File {
                    file: ".terraform.lock.hcl".into(),
                },
                DetectionPattern::File {
                    file: "main.tf".into(),
                },
                DetectionPattern::File {
                    file: "*.tf".into(),
                },
            ],
            version_from: None,
            suggests: vec!["tflint".into(), "terraform-docs".into()],
            category: ToolCategory::Ops,
        },
        DetectionRule {
            name: "pre-commit".into(),
            detect: vec![DetectionPattern::File {
                file: ".pre-commit-config.yaml".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Dev,
        },
        DetectionRule {
            name: "make".into(),
            detect: vec![
                DetectionPattern::File {
                    file: "Makefile".into(),
                },
                DetectionPattern::File {
                    file: "makefile".into(),
                },
                DetectionPattern::File {
                    file: "GNUmakefile".into(),
                },
            ],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
        DetectionRule {
            name: "just".into(),
            detect: vec![DetectionPattern::File {
                file: "Justfile".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Build,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Review P0 #2 — hostile filename with newline + `[section]` must
    /// not become a detection source string. The file-glob path is the
    /// only attacker-controllable detection input; rule-author literals
    /// are trusted.
    #[test]
    fn rejects_hostile_glob_match_filename() {
        let tmp = tempdir().unwrap();
        // Filename literally containing newline + section header.
        fs::write(tmp.path().join("x.tf\n[packages]\nbad = true\n.tf"), "").unwrap();
        let rule = DetectionRule {
            name: "terraform".into(),
            detect: vec![DetectionPattern::File {
                file: "*.tf".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Ops,
        };
        // Source MUST be None (no fallback to partial sanitization).
        assert!(rule_match_source(tmp.path(), &rule).is_none());
    }

    #[test]
    fn accepts_well_formed_filename_glob() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("main.tf"), "").unwrap();
        let rule = DetectionRule {
            name: "terraform".into(),
            detect: vec![DetectionPattern::File {
                file: "*.tf".into(),
            }],
            version_from: None,
            suggests: vec![],
            category: ToolCategory::Ops,
        };
        assert_eq!(
            rule_match_source(tmp.path(), &rule).as_deref(),
            Some("main.tf")
        );
    }

    #[test]
    fn sanitize_source_table() {
        assert_eq!(sanitize_source("Cargo.toml").as_deref(), Some("Cargo.toml"));
        assert_eq!(sanitize_source("k8s").as_deref(), Some("k8s"));
        assert!(sanitize_source("").is_none());
        assert!(sanitize_source("x\nbad").is_none());
        assert!(sanitize_source("has\"quote").is_none());
        assert!(sanitize_source("back\\slash").is_none());
        assert!(sanitize_source("nul\0byte").is_none());
        assert!(sanitize_source(&"x".repeat(256)).is_none());
    }
}
