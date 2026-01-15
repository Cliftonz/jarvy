use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{fs, process};

use crate::tools::{Os, current_os};

/// Default timeout for hooks in seconds (5 minutes)
pub const DEFAULT_HOOK_TIMEOUT: u64 = 300;

// ============================================================================
// Environment Variable Configuration
// ============================================================================

/// Environment variable value - can be simple string or complex with options
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum EnvValue {
    /// Complex value with additional options
    Complex {
        /// The value of the environment variable (supports template expansion)
        value: String,
        /// Description for documentation
        #[serde(default)]
        description: Option<String>,
        /// Whether to append to existing PATH-like variables
        #[serde(default)]
        append: bool,
        /// Whether this is per-tool (prefixed with tool name context)
        #[serde(default)]
        per_tool: bool,
    },
    /// Simple string value (supports template expansion)
    Simple(String),
}

impl EnvValue {
    /// Get the raw value string
    pub fn value(&self) -> &str {
        match self {
            EnvValue::Complex { value, .. } => value,
            EnvValue::Simple(s) => s,
        }
    }

    /// Check if this should append to existing values
    pub fn should_append(&self) -> bool {
        match self {
            EnvValue::Complex { append, .. } => *append,
            EnvValue::Simple(_) => false,
        }
    }
}

/// Secret variable configuration
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SecretValue {
    /// Load secret from a file
    FromFile {
        /// Path to file containing the secret
        from_file: String,
    },
    /// Prompt for secret (with optional default env var to check)
    Prompt {
        /// Environment variable to check before prompting
        #[serde(default)]
        env: Option<String>,
        /// Whether this secret is required
        #[serde(default = "default_true")]
        required: bool,
        /// Description shown when prompting
        #[serde(default)]
        description: Option<String>,
    },
    /// Simple prompt marker (just the variable name)
    Simple(String),
}

fn default_true() -> bool {
    true
}

/// Settings for environment variable generation
#[derive(Deserialize, Debug, Clone)]
pub struct EnvSettings {
    /// Target shell for rc file updates (bash, zsh, fish)
    #[serde(default)]
    pub shell: Option<String>,
    /// Whether to update shell rc files
    #[serde(default)]
    pub update_rc: bool,
    /// Whether to generate .env file
    #[serde(default = "default_true")]
    pub generate_dotenv: bool,
    /// Path for .env file (default: ./.env)
    #[serde(default = "default_dotenv_path")]
    pub dotenv_path: PathBuf,
    /// Whether to add .env to .gitignore
    #[serde(default)]
    pub add_to_gitignore: bool,
    /// Backup rc files before modification
    #[serde(default = "default_true")]
    pub backup_rc: bool,
}

fn default_dotenv_path() -> PathBuf {
    PathBuf::from(".env")
}

impl Default for EnvSettings {
    fn default() -> Self {
        Self {
            shell: None,
            update_rc: false,
            generate_dotenv: true,
            dotenv_path: default_dotenv_path(),
            add_to_gitignore: false,
            backup_rc: true,
        }
    }
}

/// Environment configuration section in jarvy.toml
#[derive(Deserialize, Debug, Clone, Default)]
pub struct EnvConfig {
    /// Regular environment variables
    #[serde(default)]
    pub vars: HashMap<String, EnvValue>,
    /// Secret variables (prompted or loaded from file)
    #[serde(default)]
    pub secrets: HashMap<String, SecretValue>,
    /// Settings for env generation
    #[serde(default)]
    pub config: EnvSettings,
    /// Per-tool environment variables
    #[serde(flatten)]
    pub tool_env: HashMap<String, ToolEnvConfig>,
}

/// Per-tool environment variables
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ToolEnvConfig {
    /// Environment variables specific to this tool
    #[serde(default)]
    pub vars: HashMap<String, EnvValue>,
}

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
// ============================================================================
// Services Configuration
// ============================================================================

/// Services configuration section in jarvy.toml
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ServicesConfig {
    /// Whether services feature is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Whether to auto-start services during jarvy setup
    #[serde(default)]
    pub auto_start: bool,
    /// Override path to docker-compose.yml (relative to project root)
    #[serde(default)]
    pub compose_file: Option<PathBuf>,
    /// Override path to Tiltfile (relative to project root)
    #[serde(default)]
    pub tilt_file: Option<PathBuf>,
    /// Whether to auto-start services in CI mode (default: false)
    #[serde(default)]
    pub start_in_ci: bool,
}

impl ServicesConfig {
    /// Returns true if services should be started during setup
    pub fn should_auto_start(&self, is_ci: bool) -> bool {
        if !self.enabled {
            return false;
        }
        if is_ci && !self.start_in_ci {
            return false;
        }
        self.auto_start
    }
}

// ============================================================================
// Hooks Configuration
// ============================================================================

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
    /// Environment variables configuration
    #[serde(default)]
    pub env: EnvConfig,
    /// Services configuration (docker-compose, tilt)
    #[serde(default)]
    pub services: ServicesConfig,
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
            || self
                .hooks
                .tool_hooks
                .values()
                .any(|h| h.post_install.is_some())
    }

    /// Get the environment configuration
    pub fn get_env(&self) -> &EnvConfig {
        &self.env
    }

    /// Get environment variables for a specific tool
    pub fn get_tool_env(&self, tool_name: &str) -> Option<&ToolEnvConfig> {
        self.env.tool_env.get(tool_name)
    }

    /// Check if any environment variables are configured
    pub fn has_env(&self) -> bool {
        !self.env.vars.is_empty()
            || !self.env.secrets.is_empty()
            || self.env.tool_env.values().any(|t| !t.vars.is_empty())
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

# Environment variables (optional)
# [env.vars]
# MY_VAR = "simple_value"
# PROJECT_ROOT = "$PWD"
# NODE_PATH = { value = "$HOME/.node/bin", append = true }
#
# [env.secrets]
# API_KEY = { env = "MY_API_KEY", required = true }
# DB_PASSWORD = { from_file = "~/.secrets/db_pass" }
#
# [env.config]
# generate_dotenv = true
# dotenv_path = ".env"
# update_rc = false
# add_to_gitignore = true
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

        assert_eq!(
            config.hooks.pre_setup,
            Some("echo 'Starting setup'".to_string())
        );
        assert_eq!(config.hooks.post_setup, Some("echo 'Done'".to_string()));
        assert_eq!(config.hooks.config.shell, "zsh");
        assert_eq!(config.hooks.config.timeout, 120);
        assert!(config.hooks.config.continue_on_error);

        let node_hooks = config
            .get_tool_hooks("node")
            .expect("node hooks should exist");
        assert_eq!(
            node_hooks.post_install,
            Some("npm install -g yarn".to_string())
        );
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

    #[test]
    fn test_env_config_parsing_simple() {
        let toml_str = r#"
[provisioner]
git = "latest"

[env.vars]
MY_VAR = "simple_value"
PROJECT_ROOT = "$PWD"
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");

        assert_eq!(config.env.vars.len(), 2);
        assert!(config.has_env());

        let my_var = config.env.vars.get("MY_VAR").expect("MY_VAR should exist");
        assert_eq!(my_var.value(), "simple_value");
        assert!(!my_var.should_append());
    }

    #[test]
    fn test_env_config_parsing_complex() {
        let toml_str = r#"
[provisioner]
git = "latest"

[env.vars]
NODE_PATH = { value = "$HOME/.node/bin", append = true, description = "Node binaries" }
SIMPLE = "just_a_value"

[env.config]
generate_dotenv = true
dotenv_path = ".env.local"
update_rc = true
add_to_gitignore = true
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");

        let node_path = config
            .env
            .vars
            .get("NODE_PATH")
            .expect("NODE_PATH should exist");
        assert_eq!(node_path.value(), "$HOME/.node/bin");
        assert!(node_path.should_append());

        assert!(config.env.config.generate_dotenv);
        assert_eq!(config.env.config.dotenv_path, PathBuf::from(".env.local"));
        assert!(config.env.config.update_rc);
        assert!(config.env.config.add_to_gitignore);
    }

    #[test]
    fn test_env_config_secrets() {
        let toml_str = r#"
[provisioner]
git = "latest"

[env.secrets]
API_KEY = { env = "MY_API_KEY", required = true }
DB_PASSWORD = { from_file = "~/.secrets/db_pass" }
OPTIONAL_KEY = { required = false, description = "Optional API key" }
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");

        assert_eq!(config.env.secrets.len(), 3);
        assert!(config.has_env());
    }

    #[test]
    fn test_env_config_defaults() {
        let toml_str = r#"
[provisioner]
git = "latest"
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");

        assert!(config.env.vars.is_empty());
        assert!(config.env.secrets.is_empty());
        assert!(config.env.config.generate_dotenv);
        assert_eq!(config.env.config.dotenv_path, PathBuf::from(".env"));
        assert!(!config.env.config.update_rc);
        assert!(!config.has_env());
    }

    #[test]
    fn test_env_settings_default() {
        let settings = EnvSettings::default();
        assert!(settings.shell.is_none());
        assert!(!settings.update_rc);
        assert!(settings.generate_dotenv);
        assert_eq!(settings.dotenv_path, PathBuf::from(".env"));
        assert!(!settings.add_to_gitignore);
        assert!(settings.backup_rc);
    }

    #[test]
    fn test_services_config_defaults() {
        let toml_str = r#"
[provisioner]
git = "latest"
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");

        assert!(!config.services.enabled);
        assert!(!config.services.auto_start);
        assert!(config.services.compose_file.is_none());
        assert!(config.services.tilt_file.is_none());
        assert!(!config.services.start_in_ci);
    }

    #[test]
    fn test_services_config_parsing() {
        let toml_str = r#"
[provisioner]
git = "latest"

[services]
enabled = true
auto_start = true
compose_file = "docker/docker-compose.yml"
start_in_ci = false
"#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse config");

        assert!(config.services.enabled);
        assert!(config.services.auto_start);
        assert_eq!(
            config.services.compose_file,
            Some(PathBuf::from("docker/docker-compose.yml"))
        );
        assert!(!config.services.start_in_ci);
    }

    #[test]
    fn test_services_should_auto_start() {
        // Test disabled services
        let disabled = ServicesConfig {
            enabled: false,
            auto_start: true,
            ..Default::default()
        };
        assert!(!disabled.should_auto_start(false));
        assert!(!disabled.should_auto_start(true));

        // Test enabled with auto_start off
        let no_auto = ServicesConfig {
            enabled: true,
            auto_start: false,
            ..Default::default()
        };
        assert!(!no_auto.should_auto_start(false));
        assert!(!no_auto.should_auto_start(true));

        // Test enabled with auto_start on, CI off
        let auto_no_ci = ServicesConfig {
            enabled: true,
            auto_start: true,
            start_in_ci: false,
            ..Default::default()
        };
        assert!(auto_no_ci.should_auto_start(false)); // not in CI
        assert!(!auto_no_ci.should_auto_start(true)); // in CI, start_in_ci is false

        // Test enabled with auto_start and start_in_ci on
        let auto_with_ci = ServicesConfig {
            enabled: true,
            auto_start: true,
            start_in_ci: true,
            ..Default::default()
        };
        assert!(auto_with_ci.should_auto_start(false));
        assert!(auto_with_ci.should_auto_start(true));
    }
}
