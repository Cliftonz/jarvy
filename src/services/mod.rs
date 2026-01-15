//! Service Management module
//!
//! This module provides functionality for:
//! - Detecting and managing project services (docker-compose, tilt)
//! - Starting, stopping, and checking status of services
//! - Integration with jarvy setup flow

mod docker_compose;
mod tilt;

pub use docker_compose::DockerComposeBackend;
pub use tilt::TiltBackend;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// Service backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceBackend {
    /// Docker Compose (docker-compose.yml or compose.yml)
    DockerCompose,
    /// Tilt (Tiltfile)
    Tilt,
}

impl ServiceBackend {
    /// Returns the human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::DockerCompose => "Docker Compose",
            Self::Tilt => "Tilt",
        }
    }

    /// Returns the default config file name(s)
    pub fn config_files(&self) -> &'static [&'static str] {
        match self {
            Self::DockerCompose => &[
                "docker-compose.yml",
                "docker-compose.yaml",
                "compose.yml",
                "compose.yaml",
            ],
            Self::Tilt => &["Tiltfile"],
        }
    }

    /// Returns the command used to check if backend is installed
    pub fn check_command(&self) -> &'static str {
        match self {
            Self::DockerCompose => "docker",
            Self::Tilt => "tilt",
        }
    }
}

impl fmt::Display for ServiceBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Error type for service operations
#[derive(Debug)]
pub enum ServiceError {
    /// Backend tool not installed
    BackendNotInstalled(ServiceBackend),
    /// Config file not found
    ConfigNotFound(ServiceBackend),
    /// Command execution failed
    CommandFailed {
        backend: ServiceBackend,
        operation: &'static str,
        stderr: String,
        exit_code: Option<i32>,
    },
    /// IO error
    IoError(std::io::Error),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendNotInstalled(backend) => {
                write!(
                    f,
                    "{} is not installed. Install it with: jarvy setup",
                    backend
                )
            }
            Self::ConfigNotFound(backend) => {
                write!(f, "No {} config file found in project", backend)
            }
            Self::CommandFailed {
                backend,
                operation,
                stderr,
                exit_code,
            } => {
                write!(f, "{} {} failed", backend, operation)?;
                if let Some(code) = exit_code {
                    write!(f, " (exit code {})", code)?;
                }
                if !stderr.is_empty() {
                    write!(f, ": {}", stderr)?;
                }
                Ok(())
            }
            Self::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ServiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ServiceError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

/// Result of a service operation
#[derive(Debug)]
pub struct ServiceResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Output message
    pub message: String,
    /// Backend used
    pub backend: ServiceBackend,
}

/// Service status information
#[derive(Debug)]
pub struct ServiceStatus {
    /// Backend type
    pub backend: ServiceBackend,
    /// Whether the backend is installed
    pub installed: bool,
    /// Whether services are running
    pub running: bool,
    /// Detailed status output
    pub details: String,
}

/// Trait for service backends
pub trait ServiceBackendOps {
    /// Check if the backend is installed
    fn is_installed(&self) -> bool;

    /// Find the config file in the given directory
    fn find_config(&self, dir: &Path) -> Option<PathBuf>;

    /// Start services
    fn start(&self, config_path: &Path, detach: bool) -> Result<ServiceResult, ServiceError>;

    /// Stop services
    fn stop(&self, config_path: &Path) -> Result<ServiceResult, ServiceError>;

    /// Get service status
    fn status(&self, config_path: &Path) -> Result<ServiceStatus, ServiceError>;

    /// Restart services
    fn restart(&self, config_path: &Path, detach: bool) -> Result<ServiceResult, ServiceError> {
        self.stop(config_path)?;
        self.start(config_path, detach)
    }
}

/// Detect which service backend is available in the given directory
pub fn detect_backend(dir: &Path) -> Option<(ServiceBackend, PathBuf)> {
    // Docker Compose takes priority
    let docker = DockerComposeBackend;
    if let Some(path) = docker.find_config(dir) {
        return Some((ServiceBackend::DockerCompose, path));
    }

    // Try Tilt
    let tilt = TiltBackend;
    if let Some(path) = tilt.find_config(dir) {
        return Some((ServiceBackend::Tilt, path));
    }

    None
}

/// Detect which service backend is available, with config override support
pub fn detect_backend_with_config(
    dir: &Path,
    compose_file: Option<&Path>,
    tilt_file: Option<&Path>,
) -> Option<(ServiceBackend, PathBuf)> {
    // If compose_file is explicitly set, use it
    if let Some(compose) = compose_file {
        let path = if compose.is_absolute() {
            compose.to_path_buf()
        } else {
            dir.join(compose)
        };
        if path.exists() {
            return Some((ServiceBackend::DockerCompose, path));
        }
    }

    // If tilt_file is explicitly set, use it
    if let Some(tilt) = tilt_file {
        let path = if tilt.is_absolute() {
            tilt.to_path_buf()
        } else {
            dir.join(tilt)
        };
        if path.exists() {
            return Some((ServiceBackend::Tilt, path));
        }
    }

    // Fall back to auto-detection
    detect_backend(dir)
}

/// Get the appropriate backend implementation
pub fn get_backend(backend: ServiceBackend) -> Box<dyn ServiceBackendOps> {
    match backend {
        ServiceBackend::DockerCompose => Box::new(DockerComposeBackend),
        ServiceBackend::Tilt => Box::new(TiltBackend),
    }
}

/// Check if a command exists in PATH
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run a command and capture output
fn run_command(cmd: &str, args: &[&str], working_dir: &Path) -> Result<Output, std::io::Error> {
    Command::new(cmd)
        .args(args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_service_backend_name() {
        assert_eq!(ServiceBackend::DockerCompose.name(), "Docker Compose");
        assert_eq!(ServiceBackend::Tilt.name(), "Tilt");
    }

    #[test]
    fn test_detect_docker_compose() {
        let temp = TempDir::new().unwrap();
        let compose_path = temp.path().join("docker-compose.yml");
        File::create(&compose_path)
            .unwrap()
            .write_all(b"version: '3'\n")
            .unwrap();

        let result = detect_backend(temp.path());
        assert!(result.is_some());
        let (backend, path) = result.unwrap();
        assert_eq!(backend, ServiceBackend::DockerCompose);
        assert_eq!(path, compose_path);
    }

    #[test]
    fn test_detect_compose_yml() {
        let temp = TempDir::new().unwrap();
        let compose_path = temp.path().join("compose.yml");
        File::create(&compose_path)
            .unwrap()
            .write_all(b"version: '3'\n")
            .unwrap();

        let result = detect_backend(temp.path());
        assert!(result.is_some());
        let (backend, _) = result.unwrap();
        assert_eq!(backend, ServiceBackend::DockerCompose);
    }

    #[test]
    fn test_detect_tiltfile() {
        let temp = TempDir::new().unwrap();
        let tilt_path = temp.path().join("Tiltfile");
        File::create(&tilt_path)
            .unwrap()
            .write_all(b"# Tiltfile\n")
            .unwrap();

        let result = detect_backend(temp.path());
        assert!(result.is_some());
        let (backend, path) = result.unwrap();
        assert_eq!(backend, ServiceBackend::Tilt);
        assert_eq!(path, tilt_path);
    }

    #[test]
    fn test_docker_compose_priority() {
        let temp = TempDir::new().unwrap();
        // Create both files
        File::create(temp.path().join("docker-compose.yml")).unwrap();
        File::create(temp.path().join("Tiltfile")).unwrap();

        // Docker Compose should take priority
        let result = detect_backend(temp.path());
        assert!(result.is_some());
        let (backend, _) = result.unwrap();
        assert_eq!(backend, ServiceBackend::DockerCompose);
    }

    #[test]
    fn test_no_service_found() {
        let temp = TempDir::new().unwrap();
        let result = detect_backend(temp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_config_override() {
        let temp = TempDir::new().unwrap();
        // Create compose in subdirectory
        let subdir = temp.path().join("docker");
        std::fs::create_dir(&subdir).unwrap();
        let compose_path = subdir.join("compose.yml");
        File::create(&compose_path).unwrap();

        // Without override, nothing found
        assert!(detect_backend(temp.path()).is_none());

        // With override, file found
        let result =
            detect_backend_with_config(temp.path(), Some(Path::new("docker/compose.yml")), None);
        assert!(result.is_some());
        let (backend, _) = result.unwrap();
        assert_eq!(backend, ServiceBackend::DockerCompose);
    }
}
