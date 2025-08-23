use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct CliConfig {
    pub telemetry: bool,
    pub silent: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            telemetry: true,
            silent: false,
        }
    }
}

pub(crate) fn initialize() -> CliConfig {
    // check jarvy config for the usr
    let home_dir = dirs::home_dir().expect("Failed to get home directory");

    // Create the .jarvy directory path
    let jarvy_dir = home_dir.join(".jarvy");

    // Define the path to the config.toml file
    let config_file_path = jarvy_dir.join("config.toml");

    // Create the .jarvy directory if it doesn't exist
    if !jarvy_dir.exists() {
        // Sample configuration content
        let config = CliConfig::default();

        fs::create_dir(&jarvy_dir).expect("Unable to create jarvy config file");
        println!(
            r"
        Jarvy tool collects telemetry data to help us improve your experience.
        The data collected is anonymized and used solely for analytics purposes.
        If you wish to opt-out of telemetry collection, you can disable it by adding the following line to your configuration file located at ~/.jarvy/config.toml:
        [settings]
        telemetry = false

        Thank you for using Jarvy!
                "
        );

        // Define the path to the config.toml file
        let config_file_path = jarvy_dir.join("config.toml");

        // Sample configuration content
        let config_content = r#"
                [settings]
                "#;

        // Write the content to the config.toml file
        let mut file = fs::File::create(config_file_path).expect("Unable to create config file");
        file.write_all(config_content.as_bytes())
            .expect("Unable to write content to config file");
    } else {
        // Read the existing config.toml file
        let config_content =
            fs::read_to_string(&config_file_path).expect("Unable to read config file");

        // Deserialize the config.toml file
        let config: CliConfig =
            toml::from_str(&config_content).expect("Unable to parse config file");

        return config;
    }

    CliConfig::default()
}
