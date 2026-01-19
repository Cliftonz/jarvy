//! Template management command
//!
//! Browse and use pre-built configuration templates.

use crate::output::{ExitCode, Outputable};
use crate::templates::builtin::{
    BuiltinTemplate, all_categories, get_builtin_template, list_builtin_templates,
    templates_by_category,
};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// Actions for the templates command
#[derive(Debug, Clone)]
pub enum TemplatesAction {
    /// List all available templates
    List,
    /// Show details of a specific template
    Show(String),
    /// Use a template to create jarvy.toml
    Use(String),
}

/// Options for the templates command
#[derive(Debug, Clone)]
pub struct TemplatesOptions {
    /// The action to perform
    pub action: TemplatesAction,
    /// Output file path (for 'use' action)
    pub output: Option<PathBuf>,
    /// Run setup immediately after creating config
    pub setup: bool,
    /// Non-interactive mode
    pub non_interactive: bool,
}

/// Result of listing templates
#[derive(Debug, Clone, Serialize)]
pub struct TemplatesListResult {
    pub templates: Vec<TemplateListItem>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateListItem {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tool_count: usize,
}

impl Outputable for TemplatesListResult {
    fn to_human(&self) -> String {
        let mut output = String::new();

        output.push_str("\n\x1b[1mAvailable Templates\x1b[0m\n");
        output.push_str("===================\n\n");

        // Group by category
        let categories = all_categories();
        for category in categories {
            let templates: Vec<_> = self
                .templates
                .iter()
                .filter(|t| t.category == category)
                .collect();

            if templates.is_empty() {
                continue;
            }

            output.push_str(&format!("\x1b[33m{}:\x1b[0m\n", category));
            for t in templates {
                output.push_str(&format!(
                    "  \x1b[36m{:15}\x1b[0m {} ({} tools)\n",
                    t.name, t.description, t.tool_count
                ));
            }
            output.push('\n');
        }

        output.push_str("Use: \x1b[36mjarvy templates show <name>\x1b[0m for details\n");
        output.push_str("Use: \x1b[36mjarvy templates use <name>\x1b[0m to create jarvy.toml\n");

        output
    }

    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }

    fn exit_code(&self) -> ExitCode {
        ExitCode::Ok
    }
}

/// Result of showing a template
#[derive(Debug, Clone, Serialize)]
pub struct TemplateShowResult {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tools: Vec<ToolEntry>,
    pub tool_count: usize,
    #[serde(skip)]
    pub found: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolEntry {
    pub name: String,
    pub version: String,
}

impl Outputable for TemplateShowResult {
    fn to_human(&self) -> String {
        if !self.found {
            return format!(
                "\n\x1b[31mError:\x1b[0m Template '{}' not found.\n\nRun \x1b[36mjarvy templates\x1b[0m to see available templates.\n",
                self.name
            );
        }

        let mut output = String::new();

        output.push_str(&format!("\n\x1b[1mTemplate: {}\x1b[0m\n", self.name));
        output.push_str(&"=".repeat(self.name.len() + 11));
        output.push_str("\n\n");

        output.push_str(&format!(
            "\x1b[33mDescription:\x1b[0m {}\n\n",
            self.description
        ));
        output.push_str(&format!("\x1b[33mCategory:\x1b[0m {}\n\n", self.category));

        output.push_str("\x1b[33mTools included:\x1b[0m\n");
        for tool in &self.tools {
            output.push_str(&format!(
                "  \x1b[36m•\x1b[0m {} ({})\n",
                tool.name, tool.version
            ));
        }

        output.push_str(&format!(
            "\n\x1b[33mTotal:\x1b[0m {} tools\n\n",
            self.tool_count
        ));
        output.push_str(&format!(
            "Use this template:\n  \x1b[36mjarvy templates use {}\x1b[0m\n",
            self.name
        ));

        output
    }

    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }

    fn exit_code(&self) -> ExitCode {
        if self.found {
            ExitCode::Ok
        } else {
            ExitCode::Error
        }
    }
}

/// Result of using a template
#[derive(Debug, Clone, Serialize)]
pub struct TemplateUseResult {
    pub template_name: String,
    pub output_path: Option<String>,
    pub tool_count: usize,
    pub created: bool,
    pub error: Option<String>,
}

impl Outputable for TemplateUseResult {
    fn to_human(&self) -> String {
        if let Some(ref err) = self.error {
            return format!("\n\x1b[31mError:\x1b[0m {}\n", err);
        }

        if !self.created {
            return "\n\x1b[33m!\x1b[0m jarvy.toml already exists. Use --output to specify a different path.\n".to_string();
        }

        let mut output = String::new();

        output.push_str(&format!(
            "\nUsing template: \x1b[36m{}\x1b[0m\n\n",
            self.template_name
        ));

        if let Some(ref path) = self.output_path {
            output.push_str(&format!(
                "\x1b[32m✓\x1b[0m Created {} from '{}' template ({} tools)\n\n",
                path, self.template_name, self.tool_count
            ));
        }

        output.push_str("Review and customize:\n");
        output.push_str("  \x1b[36mcode jarvy.toml\x1b[0m   # or your editor\n\n");
        output.push_str("Then install:\n");
        output.push_str("  \x1b[36mjarvy setup\x1b[0m\n");

        output
    }

    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }

    fn exit_code(&self) -> ExitCode {
        if self.error.is_some() || !self.created {
            ExitCode::Error
        } else {
            ExitCode::Ok
        }
    }
}

/// List all available templates
pub fn list_templates() -> TemplatesListResult {
    let templates: Vec<TemplateListItem> = list_builtin_templates()
        .iter()
        .map(|t| TemplateListItem {
            name: t.name.to_string(),
            description: t.description.to_string(),
            category: t.category.to_string(),
            tool_count: t.tools.len(),
        })
        .collect();

    let total = templates.len();

    TemplatesListResult { templates, total }
}

/// Show details of a specific template
pub fn show_template(name: &str) -> TemplateShowResult {
    match get_builtin_template(name) {
        Some(template) => {
            let tools: Vec<ToolEntry> = template
                .tools
                .iter()
                .map(|(name, version)| ToolEntry {
                    name: name.to_string(),
                    version: version.to_string(),
                })
                .collect();

            TemplateShowResult {
                name: template.name.to_string(),
                description: template.description.to_string(),
                category: template.category.to_string(),
                tool_count: tools.len(),
                tools,
                found: true,
            }
        }
        None => TemplateShowResult {
            name: name.to_string(),
            description: String::new(),
            category: String::new(),
            tools: vec![],
            tool_count: 0,
            found: false,
        },
    }
}

/// Use a template to create jarvy.toml
pub fn use_template(name: &str, output: Option<PathBuf>) -> TemplateUseResult {
    let template = match get_builtin_template(name) {
        Some(t) => t,
        None => {
            return TemplateUseResult {
                template_name: name.to_string(),
                output_path: None,
                tool_count: 0,
                created: false,
                error: Some(format!("Template '{}' not found", name)),
            };
        }
    };

    let output_path = output.unwrap_or_else(|| PathBuf::from("jarvy.toml"));

    // Check if file exists
    if output_path.exists() {
        return TemplateUseResult {
            template_name: name.to_string(),
            output_path: Some(output_path.display().to_string()),
            tool_count: template.tools.len(),
            created: false,
            error: None,
        };
    }

    // Generate content
    let full_template = template.to_template();
    let content = full_template.to_jarvy_toml();

    // Write file
    match fs::write(&output_path, &content) {
        Ok(()) => TemplateUseResult {
            template_name: name.to_string(),
            output_path: Some(output_path.display().to_string()),
            tool_count: template.tools.len(),
            created: true,
            error: None,
        },
        Err(e) => TemplateUseResult {
            template_name: name.to_string(),
            output_path: None,
            tool_count: 0,
            created: false,
            error: Some(format!("Failed to write file: {}", e)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_templates() {
        let result = list_templates();
        assert!(result.total >= 20);
        assert!(result.templates.iter().any(|t| t.name == "react"));
    }

    #[test]
    fn test_show_template_found() {
        let result = show_template("react");
        assert!(result.found);
        assert_eq!(result.name, "react");
        assert!(!result.tools.is_empty());
    }

    #[test]
    fn test_show_template_not_found() {
        let result = show_template("nonexistent");
        assert!(!result.found);
    }
}
