//! pip/uv Python package handler
//!
//! Provides installation of Python packages via pip with virtual environment support.
//! Supports installing from requirements files or specific package lists.

use std::path::{Path, PathBuf};

use super::common::{
    PackageError, command_exists, run_package_command, validate_package_name,
    validate_package_version,
};
use super::config::{PackageSpec, PipConfig};

/// Handler for pip package installation with virtual environment support
pub struct PipHandler {
    config: PipConfig,
    project_dir: PathBuf,
}

impl PipHandler {
    /// Create a new pip handler
    pub fn new(config: PipConfig, project_dir: PathBuf) -> Self {
        Self {
            config,
            project_dir,
        }
    }

    /// Install packages according to configuration
    pub fn install(&self) -> Result<(), PackageError> {
        // Create virtual environment if configured
        let venv_path = if let Some(ref venv) = self.config.venv {
            let path = self.project_dir.join(venv);
            if self.config.create_venv && !path.exists() {
                self.create_venv(&path)?;
            }
            Some(path)
        } else {
            None
        };

        // Determine pip executable
        let pip = self.get_pip_executable(&venv_path);

        // Check if pip is available
        let pip_str = pip.to_string_lossy();
        if !command_exists(&pip_str) && venv_path.is_none() {
            return Err(PackageError::PackageManagerNotInstalled("pip".to_string()));
        }

        if self.config.from_lockfile {
            self.install_from_lockfile(&pip)?;
        } else if !self.config.packages.is_empty() {
            self.install_packages(&pip)?;
        } else {
            println!("    No pip packages configured");
        }

        // Show activation hint
        if self.config.activate_hint {
            if let Some(ref venv) = venv_path {
                println!();
                println!("    Virtual environment created at: {}", venv.display());
                #[cfg(windows)]
                println!("    Activate with: {}\\Scripts\\activate", venv.display());
                #[cfg(not(windows))]
                println!("    Activate with: source {}/bin/activate", venv.display());
            }
        }

        Ok(())
    }

    /// Get the pip executable path (from venv or system)
    fn get_pip_executable(&self, venv_path: &Option<PathBuf>) -> PathBuf {
        if let Some(venv) = venv_path {
            #[cfg(windows)]
            {
                venv.join("Scripts").join("pip.exe")
            }
            #[cfg(not(windows))]
            {
                venv.join("bin").join("pip")
            }
        } else {
            PathBuf::from("pip3")
        }
    }

    /// Get the python executable path (from venv or system)
    fn get_python_executable(&self) -> &str {
        // Could be extended to use self.config.python_version
        "python3"
    }

    /// Create a virtual environment at the specified path
    fn create_venv(&self, path: &Path) -> Result<(), PackageError> {
        println!("    Creating virtual environment at {}...", path.display());

        let python = self.get_python_executable();

        // Check if python is available
        if !command_exists(python) {
            return Err(PackageError::PackageManagerNotInstalled(python.to_string()));
        }

        let mut args = vec!["-m", "venv"];

        if self.config.system_site_packages {
            args.push("--system-site-packages");
        }

        let path_str = path.to_string_lossy();
        args.push(&path_str);

        run_package_command(python, &args, &self.project_dir)
            .map_err(|e| PackageError::VenvCreationFailed(e.to_string()))
    }

    /// Install packages from requirements file
    fn install_from_lockfile(&self, pip: &Path) -> Result<(), PackageError> {
        let lockfile = self
            .config
            .lockfile
            .as_deref()
            .unwrap_or("requirements.txt");

        let lockfile_path = self.project_dir.join(lockfile);
        if !lockfile_path.exists() {
            return Err(PackageError::LockfileNotFound(lockfile.to_string()));
        }

        let pip_str = pip.to_string_lossy();
        run_package_command(&pip_str, &["install", "-r", lockfile], &self.project_dir)
    }

    /// Install specific packages from configuration
    fn install_packages(&self, pip: &Path) -> Result<(), PackageError> {
        // Validate names + versions before building argv (see npm handler).
        for (name, spec) in &self.config.packages {
            if spec.is_optional() {
                continue;
            }
            validate_package_name(name, "[pip]")?;
            validate_package_version(spec.version(), "[pip]")?;
        }

        let packages: Vec<String> = self
            .config
            .packages
            .iter()
            .filter(|(_, spec)| !spec.is_optional())
            .map(|(name, spec)| format_pip_spec(name, spec))
            .collect();

        if packages.is_empty() {
            println!("    No required pip packages to install");
            return Ok(());
        }

        let mut args: Vec<&str> = vec!["install"];
        args.extend(packages.iter().map(|s| s.as_str()));

        let pip_str = pip.to_string_lossy();
        run_package_command(&pip_str, &args, &self.project_dir)
    }
}

/// Format a package name with version specifier for pip install
fn format_pip_spec(name: &str, spec: &PackageSpec) -> String {
    let version = spec.version();
    if version == "latest" {
        name.to_string()
    } else if version.starts_with(">=")
        || version.starts_with("<=")
        || version.starts_with("==")
        || version.starts_with("~=")
        || version.starts_with("!=")
    {
        // Version already has operator
        format!("{}{}", name, version)
    } else {
        // Assume exact version
        format!("{}=={}", name, version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_pip_spec_latest() {
        let spec = PackageSpec::Version("latest".to_string());
        assert_eq!(format_pip_spec("pytest", &spec), "pytest");
    }

    #[test]
    fn test_format_pip_spec_with_operator() {
        let spec = PackageSpec::Version(">=7.0".to_string());
        assert_eq!(format_pip_spec("pytest", &spec), "pytest>=7.0");
    }

    #[test]
    fn test_format_pip_spec_exact() {
        let spec = PackageSpec::Version("7.0.0".to_string());
        assert_eq!(format_pip_spec("pytest", &spec), "pytest==7.0.0");
    }

    #[test]
    fn test_format_pip_spec_compatible() {
        let spec = PackageSpec::Version("~=7.0".to_string());
        assert_eq!(format_pip_spec("pytest", &spec), "pytest~=7.0");
    }
}
