//! npm/yarn/pnpm package handler
//!
//! Provides installation of Node.js packages via npm, yarn, or pnpm.
//! Supports both installing from lock files and installing specific packages.

use std::path::PathBuf;

use super::common::{
    PackageError, command_exists, run_package_command, validate_package_name,
    validate_package_version,
};
use super::config::{NpmConfig, NpmPackageManager, PackageSpec};

/// Handler for npm/yarn/pnpm package installation
pub struct NpmHandler {
    config: NpmConfig,
    project_dir: PathBuf,
}

impl NpmHandler {
    /// Create a new npm handler
    pub fn new(config: NpmConfig, project_dir: PathBuf) -> Self {
        Self {
            config,
            project_dir,
        }
    }

    /// Install packages according to configuration
    pub fn install(&self) -> Result<(), PackageError> {
        let pm = self.detect_package_manager();

        // Check if package manager is available
        if !command_exists(pm.command()) {
            return Err(PackageError::PackageManagerNotInstalled(
                pm.command().to_string(),
            ));
        }

        if self.config.from_lockfile {
            self.install_from_lockfile(pm)?;
        } else if !self.config.packages.is_empty() {
            self.install_packages(pm)?;
        } else {
            println!("    No npm packages configured");
        }

        Ok(())
    }

    /// Detect which package manager to use based on lock files or config
    fn detect_package_manager(&self) -> NpmPackageManager {
        // Use explicit config if provided
        if let Some(pm) = self.config.package_manager {
            return pm;
        }

        // Auto-detect from lock files
        if self.project_dir.join("pnpm-lock.yaml").exists() {
            NpmPackageManager::Pnpm
        } else if self.project_dir.join("yarn.lock").exists() {
            NpmPackageManager::Yarn
        } else {
            NpmPackageManager::Npm
        }
    }

    /// Install packages from existing lock file
    fn install_from_lockfile(&self, pm: NpmPackageManager) -> Result<(), PackageError> {
        let args: Vec<&str> = match pm {
            NpmPackageManager::Npm => vec!["ci"],
            NpmPackageManager::Yarn => vec!["install", "--frozen-lockfile"],
            NpmPackageManager::Pnpm => vec!["install", "--frozen-lockfile"],
        };

        run_package_command(pm.command(), &args, &self.project_dir)
    }

    /// Install specific packages from configuration
    fn install_packages(&self, pm: NpmPackageManager) -> Result<(), PackageError> {
        // Validate every name + version BEFORE building the argv. A malicious
        // jarvy.toml that ships `--registry=http://attacker = "..."` or
        // `git+https://attacker/x.git = "1.0"` is refused here, not by the
        // package manager (which would happily honor the flag/URL).
        for (name, spec) in &self.config.packages {
            if spec.is_optional() {
                continue;
            }
            validate_package_name(name, "[npm]")?;
            validate_package_version(spec.version(), "[npm]")?;
        }

        let packages: Vec<String> = self
            .config
            .packages
            .iter()
            .filter(|(_, spec)| !spec.is_optional())
            .map(|(name, spec)| format_package_spec(name, spec))
            .collect();

        if packages.is_empty() {
            println!("    No required npm packages to install");
            return Ok(());
        }

        let install_cmd = match pm {
            NpmPackageManager::Npm => "install",
            NpmPackageManager::Yarn => "add",
            NpmPackageManager::Pnpm => "add",
        };

        let mut args: Vec<&str> = vec![install_cmd];
        args.extend(packages.iter().map(|s| s.as_str()));

        run_package_command(pm.command(), &args, &self.project_dir)
    }
}

/// Format a package name with version specifier for npm install
fn format_package_spec(name: &str, spec: &PackageSpec) -> String {
    let version = spec.version();
    if version == "latest" {
        name.to_string()
    } else {
        format!("{}@{}", name, version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_package_spec_latest() {
        let spec = PackageSpec::Version("latest".to_string());
        assert_eq!(format_package_spec("typescript", &spec), "typescript");
    }

    #[test]
    fn test_format_package_spec_version() {
        let spec = PackageSpec::Version("^5.0".to_string());
        assert_eq!(format_package_spec("typescript", &spec), "typescript@^5.0");
    }

    #[test]
    fn test_detect_package_manager_default() {
        let config = NpmConfig::default();
        let handler = NpmHandler::new(config, PathBuf::from("/tmp/nonexistent"));
        let pm = handler.detect_package_manager();
        assert_eq!(pm, NpmPackageManager::Npm);
    }

    #[test]
    fn test_detect_package_manager_explicit() {
        let config = NpmConfig {
            package_manager: Some(NpmPackageManager::Pnpm),
            ..Default::default()
        };
        let handler = NpmHandler::new(config, PathBuf::from("/tmp/nonexistent"));
        let pm = handler.detect_package_manager();
        assert_eq!(pm, NpmPackageManager::Pnpm);
    }
}
