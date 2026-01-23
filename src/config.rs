use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{fs, process};

use crate::roles::definition::{RoleAssignment, RolesConfig};
use crate::team::Extends;
use crate::telemetry;
use crate::tools::{Os, current_os};

/// Default timeout for hooks in seconds (5 minutes)
pub const DEFAULT_HOOK_TIMEOUT: u64 = 300;

// ============================================================================
// Environment Variable Configuration
// ============================================================================

/// Environment variable value - can be simple string or complex with options
#[derive(Deserialize, Serialize, Debug, Clone)]
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
    #[allow(dead_code)] // Public API for env value manipulation
    pub fn should_append(&self) -> bool {
        match self {
            EnvValue::Complex { append, .. } => *append,
            EnvValue::Simple(_) => false,
        }
    }
}

/// Secret variable configuration
#[derive(Deserialize, Serialize, Debug, Clone)]
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
#[derive(Deserialize, Serialize, Debug, Clone)]
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
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
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
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ToolEnvConfig {
    /// Environment variables specific to this tool
    #[serde(default)]
    pub vars: HashMap<String, EnvValue>,
}

/// Configuration for a single hook
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ToolHooks {
    /// Script to run after this tool is installed
    #[serde(default)]
    pub post_install: Option<String>,
}

/// Settings for hook execution
#[derive(Deserialize, Serialize, Debug, Clone)]
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

// ============================================================================
// Services Configuration
// ============================================================================

/// Services configuration section in jarvy.toml
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
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

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
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
    /// Parent configs to extend (URL or local path)
    #[serde(default)]
    #[allow(dead_code)] // Used during config inheritance resolution
    pub extends: Option<Extends>,
    /// Role assignment for this config (single or multiple roles)
    /// Use `role = "name"` for single role or `role = ["a", "b"]` for multiple
    #[serde(default)]
    pub role: Option<RoleAssignment>,
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
    /// Role definitions section
    #[serde(default, rename = "roles")]
    pub roles_config: RolesConfig,
    /// Network/proxy configuration
    #[serde(default)]
    #[allow(dead_code)] // Used for proxy configuration in corporate environments
    pub network: crate::network::NetworkConfig,
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

#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
pub enum ToolConfig {
    Detailed {
        version: String,
        version_manager: Option<bool>,
        use_sudo: Option<bool>,
    },
    Simple(String),
}

/// Build a Tool from config, returning (key, tool) with minimal cloning.
/// This helper consolidates tool construction logic and reduces redundant clones.
fn build_tool_entry(name: &str, config: &ToolConfig) -> (String, Tool) {
    let name_owned = name.to_string();
    let (version, version_manager, use_sudo) = match config {
        ToolConfig::Detailed {
            version,
            version_manager,
            use_sudo,
        } => (version.clone(), version_manager.unwrap_or(true), *use_sudo),
        ToolConfig::Simple(version) => (version.clone(), true, None),
    };

    let tool = Tool {
        name: name_owned.clone(),
        version,
        version_manager,
        use_sudo,
    };

    (name_owned, tool)
}

impl Config {
    pub fn new(config_path: &str) -> Self {
        let config_content = match fs::read_to_string(config_path) {
            Ok(content) => content,
            Err(e) => {
                telemetry::config_parse_error(config_path, &e.to_string());
                println!("Failed to read config file at: {}", config_path);
                process::exit(crate::error_codes::CONFIG_ERROR);
            }
        };

        match toml::from_str::<Config>(&config_content) {
            Ok(config) => {
                // Emit telemetry on successful load
                telemetry::config_loaded(
                    config_path,
                    config.tools.len(),
                    config.has_hooks(),
                    config.has_env(),
                    config.services.enabled,
                );
                config
            }
            Err(e) => {
                telemetry::config_parse_error(config_path, &e.to_string());
                println!("Failed to parse config file: {}", e);
                process::exit(crate::error_codes::CONFIG_ERROR);
            }
        }
    }

    pub fn get_tool_configs(&self) -> HashMap<String, Tool> {
        self.tools
            .iter()
            .map(|(name, config)| build_tool_entry(name, config))
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
    #[allow(dead_code)] // Public API for tool-specific environment access
    pub fn get_tool_env(&self, tool_name: &str) -> Option<&ToolEnvConfig> {
        self.env.tool_env.get(tool_name)
    }

    /// Check if any environment variables are configured
    pub fn has_env(&self) -> bool {
        !self.env.vars.is_empty()
            || !self.env.secrets.is_empty()
            || self.env.tool_env.values().any(|t| !t.vars.is_empty())
    }

    /// Get the roles configuration
    pub fn get_roles_config(&self) -> &RolesConfig {
        &self.roles_config
    }

    /// Check if any roles are defined
    pub fn has_roles(&self) -> bool {
        !self.roles_config.roles.is_empty()
    }

    /// Get assigned role(s) if any
    pub fn get_assigned_roles(&self) -> Option<Vec<&str>> {
        self.role.as_ref().map(|r| r.as_vec())
    }

    /// Check if a role is assigned
    #[allow(dead_code)] // Public API for role configuration access
    pub fn has_assigned_role(&self) -> bool {
        self.role.as_ref().map(|r| !r.is_empty()).unwrap_or(false)
    }

    /// Get tool configs with roles applied
    /// This merges role tools with directly configured tools
    /// Direct tools override role tools
    #[allow(dead_code)] // Public API for role-based tool configuration
    pub fn get_tool_configs_with_roles(&self) -> HashMap<String, Tool> {
        use crate::roles::resolver::RoleResolver;

        let mut result = HashMap::new();

        // If roles are assigned, resolve and add those tools first
        if let Some(role_assignment) = &self.role {
            let role_names = role_assignment.as_vec();
            if !role_names.is_empty() && self.has_roles() {
                let mut resolver = RoleResolver::new(&self.roles_config);
                if let Ok(resolved) = resolver.resolve_multiple(&role_names) {
                    for (name, tool) in resolved.tools {
                        result.insert(
                            name.clone(),
                            Tool {
                                name,
                                version: tool.version,
                                version_manager: tool.version_manager,
                                use_sudo: tool.use_sudo,
                            },
                        );
                    }
                }
            }
        }

        // Direct tools override role tools - use helper for minimal cloning
        for (name, config) in &self.tools {
            let (key, tool) = build_tool_entry(name, config);
            result.insert(key, tool);
        }

        result
    }

    /// Get tool configs with an optional CLI role override
    /// If role_override is Some, it temporarily replaces the config's role assignment
    /// This is used by the --role flag in the setup command
    pub fn get_tool_configs_with_role_override(
        &self,
        role_override: Option<&str>,
    ) -> HashMap<String, Tool> {
        use crate::roles::resolver::RoleResolver;

        let mut result = HashMap::new();

        // Determine which role(s) to use: CLI override takes precedence
        // Avoid cloning self.role by computing role_names directly
        let role_names: Vec<&str> = match (role_override, &self.role) {
            (Some(name), _) => vec![name],
            (None, Some(assignment)) => assignment.as_vec(),
            (None, None) => vec![],
        };

        // If roles are assigned, resolve and add those tools first
        if !role_names.is_empty() && self.has_roles() {
            let mut resolver = RoleResolver::new(&self.roles_config);
            if let Ok(resolved) = resolver.resolve_multiple(&role_names) {
                for (name, tool) in resolved.tools {
                    // Move name into Tool.name, only clone for HashMap key
                    result.insert(
                        name.clone(),
                        Tool {
                            name,
                            version: tool.version,
                            version_manager: tool.version_manager,
                            use_sudo: tool.use_sudo,
                        },
                    );
                }
            }
        }

        // Direct tools override role tools - use helper for minimal cloning
        for (name, config) in &self.tools {
            let (key, tool) = build_tool_entry(name, config);
            result.insert(key, tool);
        }

        result
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
