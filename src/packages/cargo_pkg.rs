//! cargo binary installation handler
//!
//! Provides installation of Rust binaries via `cargo install`.
//! Supports version pinning and feature selection.

use super::common::{
    PackageError, command_exists, run_package_command, validate_package_name,
    validate_package_version,
};
use super::config::{CargoConfig, PackageSpec};

/// Handler for cargo binary installation
pub struct CargoHandler {
    config: CargoConfig,
}

impl CargoHandler {
    /// Create a new cargo handler
    pub fn new(config: CargoConfig) -> Self {
        Self { config }
    }

    /// Install all configured cargo binaries
    pub fn install(&self) -> Result<(), PackageError> {
        // Check if cargo is available
        if !command_exists("cargo") {
            return Err(PackageError::PackageManagerNotInstalled(
                "cargo".to_string(),
            ));
        }

        if self.config.packages.is_empty() {
            println!("    No cargo packages configured");
            return Ok(());
        }

        for (name, spec) in &self.config.packages {
            if spec.is_optional() {
                continue;
            }

            if let Err(e) = self.install_crate(name, spec) {
                eprintln!("    Warning: Failed to install {}: {}", name, e);
            }
        }

        Ok(())
    }

    /// Install a single crate
    fn install_crate(&self, name: &str, spec: &PackageSpec) -> Result<(), PackageError> {
        // Reject names that look like cargo flags (`--git`, `--root`) or
        // direct-URL deps (`git+https://attacker/x.git`) before they hit
        // `cargo install`. cargo would happily honor these.
        validate_package_name(name, "[cargo]")?;
        validate_package_version(spec.version(), "[cargo]")?;
        for feature in spec.features() {
            validate_package_name(feature, "[cargo features]")?;
        }

        println!("    Installing {}...", name);

        let mut args = vec!["install", name];

        // Add version if not "latest"
        let version = spec.version();
        if version != "latest" {
            args.push("--version");
            args.push(version);
        }

        // Add features if specified
        let features = spec.features();
        let features_str: String;
        if !features.is_empty() {
            features_str = features.join(",");
            args.push("--features");
            args.push(&features_str);
        }

        // Add --locked flag if configured
        if self.config.locked {
            args.push("--locked");
        }

        // Use current directory (cargo install is global)
        let current_dir = std::env::current_dir().map_err(PackageError::Io)?;
        run_package_command("cargo", &args, &current_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_cargo_handler_empty() {
        let config = CargoConfig::default();
        let handler = CargoHandler::new(config);
        // Just verify it doesn't panic
        assert!(handler.config.packages.is_empty());
    }

    #[test]
    fn test_cargo_config_with_packages() {
        let mut packages = HashMap::new();
        packages.insert(
            "cargo-watch".to_string(),
            PackageSpec::Version("latest".to_string()),
        );
        packages.insert(
            "cargo-nextest".to_string(),
            PackageSpec::Version("0.9".to_string()),
        );

        let config = CargoConfig {
            packages,
            locked: true,
        };

        let handler = CargoHandler::new(config);
        assert!(handler.config.locked);
        assert_eq!(handler.config.packages.len(), 2);
    }
}
