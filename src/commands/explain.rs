//! Explain a tool's details, installation methods, and configuration context
//!
//! Shows comprehensive information about a tool: what it is, how it's installed,
//! which roles include it, dependencies, and default hooks.

use crate::output::{ExitCode, Outputable, colors, header, subheader};
use crate::tools::spec::{
    generate_tool_index, get_tool_default_hook, get_tool_dependencies,
    get_tool_flexible_dependencies, get_tool_spec,
};
use serde::Serialize;

/// Result of explaining a tool
#[derive(Debug, Clone, Serialize)]
pub struct ExplainResult {
    pub name: String,
    pub command: String,
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub platforms: Vec<PlatformInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flexible_dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_hook: Option<HookInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub provided_by_roles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_version: Option<String>,
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub method: String,
    pub package: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HookInfo {
    pub description: String,
}

impl Outputable for ExplainResult {
    fn to_human(&self) -> String {
        if !self.found {
            return format!(
                "Unknown tool: '{}'\n\nRun 'jarvy search {}' to find similar tools.",
                self.name, self.name
            );
        }

        let mut out = String::new();
        out.push_str(&header(&format!("Tool: {}", self.name)));
        out.push('\n');

        if let Some(desc) = &self.description {
            out.push_str(&format!("\n{}\n", desc));
        }

        out.push_str(&format!(
            "\n  {}Command:{} {}\n",
            colors::DIM,
            colors::RESET,
            self.command
        ));

        if self.installed {
            out.push_str(&format!(
                "  {}Status:{}  {}Installed{}\n",
                colors::DIM,
                colors::RESET,
                colors::GREEN,
                colors::RESET
            ));
        } else {
            out.push_str(&format!(
                "  {}Status:{}  {}Not installed{}\n",
                colors::DIM,
                colors::RESET,
                colors::YELLOW,
                colors::RESET
            ));
        }

        if let Some(ver) = &self.configured_version {
            out.push_str(&format!(
                "  {}Configured:{} {}\n",
                colors::DIM,
                colors::RESET,
                ver
            ));
        }

        if let Some(cat) = &self.category {
            out.push_str(&format!(
                "  {}Category:{} {}\n",
                colors::DIM,
                colors::RESET,
                cat
            ));
        }

        // Platforms
        if !self.platforms.is_empty() {
            out.push_str(&subheader("Installation Methods"));
            out.push('\n');
            for p in &self.platforms {
                out.push_str(&format!("  {} {} ({})\n", p.os, p.package, p.method));
            }
        }

        // Dependencies
        if !self.dependencies.is_empty() {
            out.push_str(&subheader("Dependencies (required)"));
            out.push('\n');
            for d in &self.dependencies {
                out.push_str(&format!("  - {}\n", d));
            }
        }

        if !self.flexible_dependencies.is_empty() {
            out.push_str(&subheader("Dependencies (one of)"));
            out.push('\n');
            for d in &self.flexible_dependencies {
                out.push_str(&format!("  - {}\n", d));
            }
        }

        // Default hook
        if let Some(hook) = &self.default_hook {
            out.push_str(&subheader("Default Hook"));
            out.push_str(&format!("\n  {}\n", hook.description));
        }

        // Roles
        if !self.provided_by_roles.is_empty() {
            out.push_str(&subheader("Provided by Roles"));
            out.push('\n');
            for r in &self.provided_by_roles {
                out.push_str(&format!("  - {}\n", r));
            }
        }

        // Usage hint
        out.push_str(&format!(
            "\n{}Tip:{} Add to jarvy.toml:\n  [provisioner]\n  {} = \"latest\"\n",
            colors::DIM,
            colors::RESET,
            self.name
        ));

        out
    }

    fn exit_code(&self) -> ExitCode {
        if self.found {
            ExitCode::Ok
        } else {
            ExitCode::Warning
        }
    }
}

/// Run the explain command for a given tool name
pub fn run_explain(tool_name: &str, config_path: Option<&str>) -> ExplainResult {
    let spec = get_tool_spec(tool_name);
    let index = generate_tool_index();
    let tool_entry = index.tools.iter().find(|t| t.name == tool_name);

    let Some(spec) = spec else {
        return ExplainResult {
            name: tool_name.to_string(),
            command: String::new(),
            found: false,
            description: None,
            platforms: vec![],
            dependencies: vec![],
            flexible_dependencies: vec![],
            default_hook: None,
            provided_by_roles: vec![],
            configured_version: None,
            installed: false,
            category: None,
        };
    };

    // Gather platform info
    let mut platforms = Vec::new();
    if let Some(entry) = tool_entry {
        if let Some(ref macos) = entry.macos {
            if let Some(ref brew) = macos.brew {
                platforms.push(PlatformInfo {
                    os: "macOS".to_string(),
                    method: "brew".to_string(),
                    package: brew.to_string(),
                });
            }
            if let Some(ref cask) = macos.cask {
                platforms.push(PlatformInfo {
                    os: "macOS".to_string(),
                    method: "cask".to_string(),
                    package: cask.to_string(),
                });
            }
        }
        if let Some(ref linux) = entry.linux {
            if let Some(ref apt) = linux.apt {
                platforms.push(PlatformInfo {
                    os: "Linux".to_string(),
                    method: "apt".to_string(),
                    package: apt.to_string(),
                });
            }
            if let Some(ref brew) = linux.brew {
                platforms.push(PlatformInfo {
                    os: "Linux".to_string(),
                    method: "brew".to_string(),
                    package: brew.to_string(),
                });
            }
        }
        if let Some(ref win) = entry.windows {
            if let Some(ref winget) = win.winget {
                platforms.push(PlatformInfo {
                    os: "Windows".to_string(),
                    method: "winget".to_string(),
                    package: winget.to_string(),
                });
            }
            if let Some(ref choco) = win.choco {
                platforms.push(PlatformInfo {
                    os: "Windows".to_string(),
                    method: "choco".to_string(),
                    package: choco.to_string(),
                });
            }
        }
    }

    // Dependencies
    let dependencies: Vec<String> = get_tool_dependencies(tool_name)
        .iter()
        .map(|s| s.to_string())
        .collect();

    let flexible_dependencies: Vec<String> = get_tool_flexible_dependencies(tool_name)
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Default hook
    let default_hook = get_tool_default_hook(tool_name).map(|h| HookInfo {
        description: h.description.to_string(),
    });

    // Check which roles provide this tool
    let mut provided_by_roles = Vec::new();
    let mut configured_version = None;

    if let Some(path) = config_path {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(config) = toml::from_str::<toml::Value>(&content) {
                // Check roles
                if let Some(roles) = config.get("roles").and_then(|r| r.as_table()) {
                    for (role_name, role_def) in roles {
                        if let Some(tools) = role_def.get("tools").and_then(|t| t.as_array()) {
                            for t in tools {
                                if t.as_str() == Some(tool_name) {
                                    provided_by_roles.push(role_name.clone());
                                }
                            }
                        }
                    }
                }

                // Check configured version
                if let Some(provisioner) = config.get("provisioner").and_then(|p| p.as_table()) {
                    if let Some(tool_config) = provisioner.get(tool_name) {
                        configured_version = match tool_config {
                            toml::Value::String(s) => Some(s.clone()),
                            toml::Value::Table(t) => t
                                .get("version")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            _ => None,
                        };
                    }
                }
            }
        }
    }

    // Check if installed
    let installed = crate::tools::common::has(spec.command);

    let category = tool_entry.and_then(|e| e.category.as_ref()).cloned();

    ExplainResult {
        name: tool_name.to_string(),
        command: spec.command.to_string(),
        found: true,
        description: None, // ToolSpec doesn't carry doc comments at runtime
        platforms,
        dependencies,
        flexible_dependencies,
        default_hook,
        provided_by_roles,
        configured_version,
        installed,
        category,
    }
}
