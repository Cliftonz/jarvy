use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::{fs, process};

use crate::tools::{Os, current_os};

/// Default timeout for hooks in seconds (5 minutes)
pub const DEFAULT_HOOK_TIMEOUT: u64 = 300;

/// Configuration for a single hook
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ToolHooks {
    /// Script to run after this tool is installed
    #[serde(default)]
    pub post_install: Option<String>,
}

/// Settings for hook execution
#[derive(Deserialize, Debug, Clone)]
pub struct HookSettings {
    /// Shell to use for running hooks (bash, zsh, sh, powershell)
    #[serde(default = "default_shell")]
    pub shell: String,
    /// Timeout in seconds for each hook (default: 300 = 5 minutes)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Whether to continue setup if a hook fails
    #[serde(default)]
    pub continue_on_error: bool,
}

fn default_shell() -> String {
    #[cfg(windows)]
    {
        "powershell".to_string()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

fn default_timeout() -> u64 {
    DEFAULT_HOOK_TIMEOUT
}

impl Default for HookSettings {
    fn default() -> Self {
        Self {
            shell: default_shell(),
            timeout: DEFAULT_HOOK_TIMEOUT,
            continue_on_error: false,
        }
    }
}

/// Configuration for all hooks in jarvy.toml
#[derive(Deserialize, Debug, Clone, Default)]
pub struct HooksConfig {
    /// Script to run before any tool installation
    #[serde(default)]
    pub pre_setup: Option<String>,
    /// Script to run after all tools are installed
    #[serde(default)]
    pub post_setup: Option<String>,
    /// Hook settings (shell, timeout, etc.)
    #[serde(default)]
    pub config: HookSettings,
    /// Per-tool hooks, keyed by tool name
    #[serde(flatten)]
    pub tool_hooks: HashMap<String, ToolHooks>,
}

#[derive(Deserialize)]
pub struct Config {
    #[serde(rename = "provisioner")]
    tools: HashMap<String, ToolConfig>,
    #[serde(default)]
    privileges: Option<PrivilegeConfig>,
    /// Hooks configuration section
    #[serde(default)]
    pub hooks: HooksConfig,
}

#[derive(Deserialize, Debug, Default)]
pub struct PrivilegeConfig {
    // Global default; if None, a sensible per-OS default is used
    #[serde(default)]
    pub use_sudo: Option<bool>,
    // Per-OS overrides, e.g., { linux = true, macos = false }
    #[serde(default)]
    pub per_os: HashMap<Os, bool>,
}

impl PrivilegeConfig {
    // Sensible defaults if nothing specified
    fn default_for(_os: Os) -> Option<bool> {
        // Returning None indicates: auto-detect per operation
        None
    }

    pub fn effective_for(&self, os: Os) -> Option<bool> {
        if let Some(v) = self.per_os.get(&os) {
            Some(*v)
        } else if let Some(global) = self.use_sudo {
            Some(global)
        } else {
            Self::default_for(os)
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ToolConfig {
    Detailed {
        version: String,
        version_manager: Option<bool>,
        use_sudo: Option<bool>,
    },
    Simple(String),
}

impl Config {
    pub fn new(config_path: &str) -> Self {
        let config_content = match fs::read_to_string(config_path) {
            Ok(content) => content,
            Err(_) => {
                println!("Failed to read config file at: {}", config_path);
                process::exit(crate::error_codes::CONFIG_ERROR);
            }
        };

        match toml::from_str(&config_content) {
            Ok(config) => config,
            Err(_) => {
                println!("Failed to parse config file. Please ensure it's in correct format.");
                process::exit(crate::error_codes::CONFIG_ERROR);
            }
        }
    }

    pub fn get_tool_configs(&self) -> HashMap<String, Tool> {
        self.tools
            .iter()
            .map(|(name, config)| {
                let tool = match config {
                    ToolConfig::Detailed {
                        version,
                        version_manager,
                        use_sudo,
                    } => Tool {
                        name: name.clone(),
                        version: version.clone(),
                        version_manager: version_manager.unwrap_or(true),
                        use_sudo: *use_sudo,
                    },
                    ToolConfig::Simple(version) => Tool {
                        name: name.clone(),
                        version: version.clone(),
                        version_manager: true,
                        use_sudo: None,
                    },
                };
                (name.clone(), tool)
            })
            .collect()
    }

    // Returns whether sudo should be used on the current OS; None => auto-detect per op
    pub fn use_sudo(&self) -> Option<bool> {
        let os = current_os();
        self.privileges
            .as_ref()
            .map(|p| p.effective_for(os))
            .unwrap_or_else(|| PrivilegeConfig::default_for(os))
    }

    /// Get the hooks configuration
    pub fn get_hooks(&self) -> &HooksConfig {
        &self.hooks
    }

    /// Get hooks for a specific tool
    pub fn get_tool_hooks(&self, tool_name: &str) -> Option<&ToolHooks> {
        self.hooks.tool_hooks.get(tool_name)
    }

    /// Check if any hooks are configured
    pub fn has_hooks(&self) -> bool {
        self.hooks.pre_setup.is_some()
            || self.hooks.post_setup.is_some()
            || self.hooks.tool_hooks.values().any(|h| h.post_install.is_some())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tool {
    pub name: String,
    pub version: String,
    pub version_manager: bool,
    pub use_sudo: Option<bool>, // carry per-tool override
}

pub fn create_default_config() {
    let default_config = r#"
[privileges]
use_sudo = true

[privileges.per_os]
linux = true
macos = false
windows = false

[provisioner]
git = "latest"
docker = "latest"

# Hook configuration (optional)
# [hooks]
# pre_setup = "echo 'Starting Jarvy setup...'"
# post_setup = "echo 'Setup complete!'"
#
# [hooks.config]
# shell = "zsh"           # or "bash", "sh", "powershell"
# timeout = 300           # seconds (default: 5 minutes)
# continue_on_error = false
#
# [hooks.node]
# post_install = "npm install -g yarn"
"#;
    let mut file = File::create("jarvy.toml").expect("Could not create file");
    file.write_all(default_config.as_bytes())
        .expect("Could not write to file");
    println!("Created jarvy.toml with default configuration");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hooks_config_parsing() {
        let toml_str = r#"
[provisioner]
git = "latest"

[hooks]
pre_setup = "echo 'Starting setup'"
post_setup = "echo 'Done'"

[hooks.config]
shell = "zsh"
timeout = 120
continue_on_error = true

[hooks.node]
post_install = "npm install -g yarn"
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");

        assert_eq!(config.hooks.pre_setup, Some("echo 'Starting setup'".to_string()));
        assert_eq!(config.hooks.post_setup, Some("echo 'Done'".to_string()));
        assert_eq!(config.hooks.config.shell, "zsh");
        assert_eq!(config.hooks.config.timeout, 120);
        assert!(config.hooks.config.continue_on_error);

        let node_hooks = config.get_tool_hooks("node").expect("node hooks should exist");
        assert_eq!(node_hooks.post_install, Some("npm install -g yarn".to_string()));
    }

    #[test]
    fn test_hooks_config_defaults() {
        let toml_str = r#"
[provisioner]
git = "latest"
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");

        assert!(config.hooks.pre_setup.is_none());
        assert!(config.hooks.post_setup.is_none());
        assert_eq!(config.hooks.config.timeout, DEFAULT_HOOK_TIMEOUT);
        assert!(!config.hooks.config.continue_on_error);
        assert!(!config.has_hooks());
    }

    #[test]
    fn test_has_hooks() {
        let toml_str = r#"
[provisioner]
git = "latest"

[hooks]
pre_setup = "echo 'hi'"
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");
        assert!(config.has_hooks());
    }

    #[test]
    fn test_hook_settings_default_shell() {
        let settings = HookSettings::default();
        // Should be the value of SHELL env var or /bin/sh on Unix, powershell on Windows
        #[cfg(not(windows))]
        {
            assert!(!settings.shell.is_empty());
        }
        #[cfg(windows)]
        {
            assert_eq!(settings.shell, "powershell");
        }
        assert_eq!(settings.timeout, DEFAULT_HOOK_TIMEOUT);
        assert!(!settings.continue_on_error);
    }
}
