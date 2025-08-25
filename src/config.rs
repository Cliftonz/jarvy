use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::{fs, process};

use crate::tools::{Os, current_os};

#[derive(Deserialize)]
pub struct Config {
    #[serde(rename = "provisioner")]
    tools: HashMap<String, ToolConfig>,
    #[serde(default)]
    privileges: Option<PrivilegeConfig>,
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
    fn default_for(os: Os) -> bool {
        match os {
            Os::Linux => true,
            Os::Macos => false,
            Os::Windows => false,
        }
    }

    pub fn effective_for(&self, os: Os) -> bool {
        if let Some(v) = self.per_os.get(&os) {
            *v
        } else if let Some(global) = self.use_sudo {
            global
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

    // Returns whether sudo should be used on the current OS
    pub fn use_sudo(&self) -> bool {
        let os = current_os();
        self.privileges
            .as_ref()
            .map(|p| p.effective_for(os))
            .unwrap_or_else(|| PrivilegeConfig::default_for(os))
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
"#;
    let mut file = File::create("jarvy.toml").expect("Could not create file");
    file.write_all(default_config.as_bytes())
        .expect("Could not write to file");
    println!("Created jarvy.toml with default configuration");
}
