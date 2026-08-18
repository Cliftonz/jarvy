//! Template schema definition
//!
//! Defines the structure for Jarvy configuration templates.

#![allow(dead_code)] // Public API for template schema

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Jarvy configuration template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Template metadata
    pub template: TemplateMeta,
    /// Tools included in this template
    #[serde(default)]
    pub tools: TemplateTools,
    /// Optional hooks configuration
    #[serde(default)]
    pub hooks: HashMap<String, TemplateHook>,
    /// Named project commands emitted into `[commands]`.
    #[serde(default)]
    pub commands: HashMap<String, String>,
}

/// Template metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMeta {
    /// Template name (e.g., "react", "go-api")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Category (e.g., "Web Development", "Backend")
    #[serde(default)]
    pub category: String,
    /// Tags for searchability
    #[serde(default)]
    pub tags: Vec<String>,
    /// Author of this template
    #[serde(default)]
    pub author: Option<String>,
    /// Template version
    #[serde(default)]
    pub version: Option<String>,
    /// Minimum Jarvy version required
    #[serde(default)]
    pub min_jarvy_version: Option<String>,
}

/// Tools in a template - can be simple string versions or detailed configs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TemplateTools {
    /// Map of tool name to version
    pub tools: HashMap<String, String>,
}

impl TemplateTools {
    /// Create a new empty TemplateTools
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Add a tool with a version
    pub fn add(&mut self, name: &str, version: &str) {
        self.tools.insert(name.to_string(), version.to_string());
    }

    /// Get the number of tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Iterate over tools
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.tools.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Hook configuration in a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateHook {
    /// Description of what this hook does
    #[serde(default)]
    pub description: Option<String>,
    /// Shell script to run
    #[serde(default)]
    pub script: Option<String>,
}

impl Template {
    /// Parse a template from TOML string
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Get the template name
    pub fn name(&self) -> &str {
        &self.template.name
    }

    /// Get the template description
    pub fn description(&self) -> &str {
        &self.template.description
    }

    /// Get the category
    pub fn category(&self) -> &str {
        &self.template.category
    }

    /// Get the number of tools in this template
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Generate jarvy.toml content from this template
    pub fn to_jarvy_toml(&self) -> String {
        let mut content = String::new();

        content.push_str(&format!("# Generated from {} template\n", self.name()));
        content.push_str(&format!("# {}\n\n", self.description()));

        content.push_str("[provisioner]\n");

        // Sort tools for consistent output
        let mut tools: Vec<_> = self.tools.iter().collect();
        tools.sort_by_key(|(name, _)| *name);

        for (name, version) in tools {
            emit_kv(&mut content, name, version);
        }

        // Add hooks if present
        if !self.hooks.is_empty() {
            content.push('\n');
            for (tool, hook) in &self.hooks {
                if let Some(ref desc) = hook.description {
                    content.push_str(&format!("# {}: {}\n", tool, desc));
                }
                if let Some(ref script) = hook.script {
                    // `[hooks.<tool>]` — tool names come from static
                    // template literals today so escaping the section
                    // header is preemptive, not exploitable. The script
                    // body stays raw inside `'''...'''` triple quotes
                    // because that's the whole point of the heredoc.
                    content.push_str(&format!("[hooks.{}]\n", tool));
                    content.push_str(&format!("script = '''\n{}\n'''\n\n", script.trim()));
                }
            }
        }

        if !self.commands.is_empty() {
            content.push_str("\n[commands]\n");
            let mut commands: Vec<_> = self.commands.iter().collect();
            commands.sort_by_key(|(name, _)| *name);
            for (name, command) in commands {
                emit_kv_quoted_key(&mut content, name, command);
            }
        }

        content
    }
}

/// Emit a single `key = "value"` line with TOML-safe VALUE escaping.
/// Prevents a future template loader (remote catalog, user-authored
/// file) whose value contains a literal `"` or newline from breaking
/// out of the string literal and injecting a `[commands]` block that
/// `jarvy run` would later execute. Not exploitable today (all
/// templates are static string literals) but closes the primitive.
///
/// Keys are emitted bare — tool / hook / command names in built-in
/// templates are alphanumeric+`-`+`_`+`:` and TOML permits bare keys
/// for that class. If a template ever ships a key with a control byte
/// or a space, that would fail bare-key TOML syntax entirely and
/// callers would notice on the re-parse test rather than here. The
/// `[commands]` section's `"pre:android"` needs explicit quoting; the
/// commands loop still routes keys through `toml::Value::String` for
/// that specific reason (colon isn't a bare-key char).
fn emit_kv(content: &mut String, key: &str, value: &str) {
    let value = toml::Value::String(value.to_string()).to_string();
    content.push_str(&format!("{key} = {value}\n"));
}

/// Same as [`emit_kv`] but quotes the KEY as well — needed for
/// `[commands]` entries like `pre:android` where the colon disqualifies
/// bare-key TOML syntax.
fn emit_kv_quoted_key(content: &mut String, key: &str, value: &str) {
    let key = toml::Value::String(key.to_string()).to_string();
    let value = toml::Value::String(value.to_string()).to_string();
    content.push_str(&format!("{key} = {value}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_from_toml() {
        let toml = r#"
[template]
name = "test"
description = "Test template"
category = "Testing"
tags = ["test", "example"]

[tools]
git = "latest"
node = "20"
"#;
        let template = Template::from_toml(toml).unwrap();
        assert_eq!(template.name(), "test");
        assert_eq!(template.description(), "Test template");
        assert_eq!(template.category(), "Testing");
        assert_eq!(template.tool_count(), 2);
    }

    #[test]
    fn test_template_to_jarvy_toml() {
        let mut tools = TemplateTools::new();
        tools.add("git", "latest");
        tools.add("node", "20");

        let template = Template {
            template: TemplateMeta {
                name: "test".to_string(),
                description: "Test template".to_string(),
                category: "Testing".to_string(),
                tags: vec![],
                author: None,
                version: None,
                min_jarvy_version: None,
            },
            tools,
            hooks: HashMap::new(),
            commands: HashMap::new(),
        };

        let content = template.to_jarvy_toml();
        assert!(content.contains("[provisioner]"));
        assert!(content.contains("git = \"latest\""));
        assert!(content.contains("node = \"20\""));
    }

    #[test]
    fn test_template_commands_render_quoted_lifecycle_keys() {
        let template = Template {
            template: TemplateMeta {
                name: "mobile".to_string(),
                description: "Mobile".to_string(),
                category: "Testing".to_string(),
                tags: vec![],
                author: None,
                version: None,
                min_jarvy_version: None,
            },
            tools: TemplateTools::new(),
            hooks: HashMap::new(),
            commands: HashMap::from([
                (
                    "pre:android".to_string(),
                    "jarvy doctor --check tools".to_string(),
                ),
                (
                    "android".to_string(),
                    "pnpm --filter mobile android".to_string(),
                ),
            ]),
        };

        let content = template.to_jarvy_toml();

        assert!(content.contains("[commands]"));
        assert!(content.contains("\"pre:android\" = \"jarvy doctor --check tools\""));
        assert!(content.contains("\"android\" = \"pnpm --filter mobile android\""));
        toml::from_str::<toml::Value>(&content).expect("rendered template must remain valid TOML");
    }

    #[test]
    fn test_template_tools_operations() {
        let mut tools = TemplateTools::new();
        assert!(tools.is_empty());

        tools.add("git", "latest");
        assert_eq!(tools.len(), 1);
        assert!(!tools.is_empty());

        tools.add("node", "20");
        assert_eq!(tools.len(), 2);
    }
}
