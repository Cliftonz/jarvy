//! Template schema definition
//!
//! Defines the structure for Jarvy configuration templates.

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
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.tools.iter()
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
        tools.sort_by_key(|(name, _)| name.as_str());

        for (name, version) in tools {
            content.push_str(&format!("{} = \"{}\"\n", name, version));
        }

        // Add hooks if present
        if !self.hooks.is_empty() {
            content.push('\n');
            for (tool, hook) in &self.hooks {
                if let Some(ref desc) = hook.description {
                    content.push_str(&format!("# {}: {}\n", tool, desc));
                }
                if let Some(ref script) = hook.script {
                    content.push_str(&format!("[hooks.{}]\n", tool));
                    content.push_str(&format!("script = '''\n{}\n'''\n\n", script.trim()));
                }
            }
        }

        content
    }
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
        };

        let content = template.to_jarvy_toml();
        assert!(content.contains("[provisioner]"));
        assert!(content.contains("git = \"latest\""));
        assert!(content.contains("node = \"20\""));
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
