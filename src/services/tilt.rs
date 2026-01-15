//! Tilt service backend

use super::{
    ServiceBackend, ServiceBackendOps, ServiceError, ServiceResult, ServiceStatus, command_exists,
    run_command,
};
use std::path::{Path, PathBuf};

/// Tilt backend implementation
pub struct TiltBackend;

impl ServiceBackendOps for TiltBackend {
    fn is_installed(&self) -> bool {
        command_exists("tilt")
    }

    fn find_config(&self, dir: &Path) -> Option<PathBuf> {
        for filename in ServiceBackend::Tilt.config_files() {
            let path = dir.join(filename);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    fn start(&self, config_path: &Path, _detach: bool) -> Result<ServiceResult, ServiceError> {
        if !self.is_installed() {
            return Err(ServiceError::BackendNotInstalled(ServiceBackend::Tilt));
        }

        let working_dir = config_path.parent().unwrap_or(Path::new("."));
        let config_file = config_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("Tiltfile");

        // tilt up with --stream=false runs in background mode
        let args = ["up", "-f", config_file, "--stream=false"];

        let output = run_command("tilt", &args, working_dir)?;

        if output.status.success() {
            Ok(ServiceResult {
                success: true,
                message: "Tilt services started. Access dashboard at http://localhost:10350"
                    .to_string(),
                backend: ServiceBackend::Tilt,
            })
        } else {
            Err(ServiceError::CommandFailed {
                backend: ServiceBackend::Tilt,
                operation: "start",
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            })
        }
    }

    fn stop(&self, config_path: &Path) -> Result<ServiceResult, ServiceError> {
        if !self.is_installed() {
            return Err(ServiceError::BackendNotInstalled(ServiceBackend::Tilt));
        }

        let working_dir = config_path.parent().unwrap_or(Path::new("."));
        let config_file = config_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("Tiltfile");

        let args = ["down", "-f", config_file];

        let output = run_command("tilt", &args, working_dir)?;

        if output.status.success() {
            Ok(ServiceResult {
                success: true,
                message: "Tilt services stopped".to_string(),
                backend: ServiceBackend::Tilt,
            })
        } else {
            Err(ServiceError::CommandFailed {
                backend: ServiceBackend::Tilt,
                operation: "stop",
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            })
        }
    }

    fn status(&self, config_path: &Path) -> Result<ServiceStatus, ServiceError> {
        if !self.is_installed() {
            return Ok(ServiceStatus {
                backend: ServiceBackend::Tilt,
                installed: false,
                running: false,
                details: "Tilt is not installed".to_string(),
            });
        }

        let working_dir = config_path.parent().unwrap_or(Path::new("."));

        // tilt status shows resource status
        let output = run_command("tilt", &["status"], working_dir)?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // If tilt status fails or returns empty, services are not running
        let running = output.status.success() && !stdout.trim().is_empty();

        Ok(ServiceStatus {
            backend: ServiceBackend::Tilt,
            installed: true,
            running,
            details: if running {
                stdout
            } else if !stderr.is_empty() {
                format!("Tilt not running: {}", stderr.trim())
            } else {
                "Tilt services not running".to_string()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_tiltfile() {
        use std::fs::File;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        File::create(temp.path().join("Tiltfile")).unwrap();

        let backend = TiltBackend;
        let result = backend.find_config(temp.path());
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("Tiltfile"));
    }

    #[test]
    fn test_find_tiltfile_not_found() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let backend = TiltBackend;
        let result = backend.find_config(temp.path());
        assert!(result.is_none());
    }
}
