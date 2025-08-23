use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::{fs, process};

#[derive(Deserialize)]
pub struct Config {
    tools: HashMap<String, ToolConfig>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ToolConfig {
    Detailed {
        version: String,
        version_manager: Option<bool>,
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
                    } => Tool {
                        name: name.clone(),
                        version: version.clone(),
                        version_manager: version_manager.unwrap_or(true),
                    },
                    ToolConfig::Simple(version) => Tool {
                        name: name.clone(),
                        version: version.clone(),
                        version_manager: true,
                    },
                };
                (name.clone(), tool)
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct Tool {
    pub name: String,
    pub version: String,
    pub version_manager: bool,
}

pub fn create_default_config() {
    let default_config = r#"
[tools]
git = "latest"
docker = "latest"
"#;
    let mut file = File::create("jarvy.toml").expect("Could not create file");
    file.write_all(default_config.as_bytes())
        .expect("Could not write to file");
    println!("Created jarvy.toml with default configuration");
}
