//! Docker Compose service backend

use super::preflight::{docker_daemon_hint, probe_container_daemon, DaemonState};
use super::{
    command_exists, run_command, ServiceBackend, ServiceBackendOps, ServiceError, ServiceResult,
    ServiceStatus,
};
use crate::observability::telemetry_gate;
use crate::telemetry;
use std::path::{Path, PathBuf};

/// Docker Compose backend implementation
pub struct DockerComposeBackend;

impl DockerComposeBackend {
    /// Check if 'docker compose' subcommand is available (modern Docker)
    fn has_docker_compose_v2() -> bool {
        std::process::Command::new("docker")
            .args(["compose", "version"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Check if legacy 'docker-compose' command is available
    fn has_docker_compose_v1() -> bool {
        command_exists("docker-compose")
    }

    /// Get the compose command prefix (either "docker compose" or "docker-compose").
    /// Memoized in a process-lifetime `OnceLock` — see the podman
    /// counterpart for the perf-F3 rationale. Cache miss cost: single
    /// `docker compose version` probe at process startup.
    fn compose_command() -> (&'static str, Vec<&'static str>) {
        static CACHED: std::sync::OnceLock<(&'static str, Vec<&'static str>)> =
            std::sync::OnceLock::new();
        CACHED
            .get_or_init(|| {
                if Self::has_docker_compose_v2() {
                    ("docker", vec!["compose"])
                } else {
                    ("docker-compose", vec![])
                }
            })
            .clone()
    }
}

impl ServiceBackendOps for DockerComposeBackend {
    fn is_installed(&self) -> bool {
        command_exists("docker") && (Self::has_docker_compose_v2() || Self::has_docker_compose_v1())
    }

    fn find_config(&self, dir: &Path) -> Option<PathBuf> {
        for filename in ServiceBackend::DockerCompose.config_files() {
            let path = dir.join(filename);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    fn check_daemon(&self) -> Result<(), ServiceError> {
        let started = std::time::Instant::now();
        let state = probe_container_daemon("docker");
        let duration_ms = started.elapsed().as_millis() as u64;
        if telemetry_gate::is_enabled() {
            // Info-level (was debug) — this is the per-invocation
            // adoption signal for the Docker vs Podman product
            // question ("of users with both installed, which do
            // they actually run against?"). At debug it was silently
            // dropped by default prod log filters.
            tracing::info!(
                event = "services.daemon_check",
                backend = "docker",
                state = ?state,
                duration_ms,
                "docker daemon preflight"
            );
        }
        match state {
            DaemonState::Running => Ok(()),
            DaemonState::Down | DaemonState::Missing | DaemonState::Timeout => {
                let (hint, hint_kind) = docker_daemon_hint();
                if telemetry_gate::is_enabled() {
                    // Distinct event for timeout so on-call can graph
                    // "hung daemon" separately from "daemon exited fast."
                    if state == DaemonState::Timeout {
                        tracing::warn!(
                            event = "services.daemon_probe_timeout",
                            backend = "docker",
                            timeout_ms = super::preflight::PROBE_TIMEOUT.as_millis() as u64,
                            duration_ms,
                            "docker daemon probe hit hard timeout, treated as down"
                        );
                    }
                    tracing::warn!(
                        event = "services.daemon_down",
                        backend = "docker",
                        state = ?state,
                        hint_kind,
                        duration_ms,
                        "docker daemon is not reachable"
                    );
                }
                Err(ServiceError::DaemonNotRunning {
                    backend: ServiceBackend::DockerCompose,
                    hint,
                })
            }
        }
    }

    fn start(&self, config_path: &Path, detach: bool) -> Result<ServiceResult, ServiceError> {
        if !self.is_installed() {
            return Err(ServiceError::BackendNotInstalled(
                ServiceBackend::DockerCompose,
            ));
        }
        self.check_daemon()?;

        let working_dir = config_path.parent().unwrap_or(Path::new("."));
        let config_file = config_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("docker-compose.yml");

        let (cmd, mut args) = Self::compose_command();
        args.extend(["-f", config_file, "up"]);
        if detach {
            args.push("-d");
        }

        let args_ref: Vec<&str> = args.to_vec();
        let output = run_command(cmd, &args_ref, working_dir)?;

        if output.status.success() {
            telemetry::service_operation("docker-compose", "start", true);
            Ok(ServiceResult {
                success: true,
                message: "Services started successfully".to_string(),
                backend: ServiceBackend::DockerCompose,
            })
        } else {
            telemetry::service_operation("docker-compose", "start", false);
            Err(ServiceError::CommandFailed {
                backend: ServiceBackend::DockerCompose,
                operation: "start",
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            })
        }
    }

    fn stop(&self, config_path: &Path) -> Result<ServiceResult, ServiceError> {
        if !self.is_installed() {
            return Err(ServiceError::BackendNotInstalled(
                ServiceBackend::DockerCompose,
            ));
        }

        let working_dir = config_path.parent().unwrap_or(Path::new("."));
        let config_file = config_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("docker-compose.yml");

        let (cmd, mut args) = Self::compose_command();
        args.extend(["-f", config_file, "down"]);

        let args_ref: Vec<&str> = args.to_vec();
        let output = run_command(cmd, &args_ref, working_dir)?;

        if output.status.success() {
            telemetry::service_operation("docker-compose", "stop", true);
            Ok(ServiceResult {
                success: true,
                message: "Services stopped successfully".to_string(),
                backend: ServiceBackend::DockerCompose,
            })
        } else {
            telemetry::service_operation("docker-compose", "stop", false);
            Err(ServiceError::CommandFailed {
                backend: ServiceBackend::DockerCompose,
                operation: "stop",
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            })
        }
    }

    fn status(&self, config_path: &Path) -> Result<ServiceStatus, ServiceError> {
        if !self.is_installed() {
            return Ok(ServiceStatus {
                backend: ServiceBackend::DockerCompose,
                installed: false,
                running: false,
                details: "Docker Compose is not installed".to_string(),
            });
        }
        match self.check_daemon() {
            Ok(()) => {}
            Err(ServiceError::DaemonNotRunning { hint, .. }) => {
                return Ok(ServiceStatus {
                    backend: ServiceBackend::DockerCompose,
                    installed: true,
                    running: false,
                    details: format!("Docker daemon is not running. {hint}"),
                });
            }
            Err(other) => {
                // Silent-fallback avoidance: any check_daemon error
                // variant other than DaemonNotRunning is unexpected
                // and would otherwise be swallowed, letting
                // `compose ps` run against an unknown daemon state
                // and reporting whatever it prints. Emit a distinct
                // event AND return an honest "unknown" ServiceStatus.
                if telemetry_gate::is_enabled() {
                    tracing::warn!(
                        event = "services.status_daemon_check_swallowed",
                        backend = "docker",
                        "unexpected error from check_daemon; reporting unknown state"
                    );
                }
                return Ok(ServiceStatus {
                    backend: ServiceBackend::DockerCompose,
                    installed: true,
                    running: false,
                    details: format!("Docker daemon state unknown: {other}"),
                });
            }
        }

        let working_dir = config_path.parent().unwrap_or(Path::new("."));
        let config_file = config_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("docker-compose.yml");

        let (cmd, mut args) = Self::compose_command();
        args.extend(["-f", config_file, "ps"]);

        let args_ref: Vec<&str> = args.to_vec();
        let output = run_command(cmd, &args_ref, working_dir)?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Check if any containers are running
        // docker compose ps shows running containers, empty or just header means nothing running
        let lines: Vec<&str> = stdout.lines().collect();
        let running = lines.len() > 1; // More than just header line

        Ok(ServiceStatus {
            backend: ServiceBackend::DockerCompose,
            installed: true,
            running,
            details: if running {
                stdout
            } else if !stderr.is_empty() {
                stderr
            } else {
                "No services running".to_string()
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_config_docker_compose_yml() {
        use std::fs::File;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        File::create(temp.path().join("docker-compose.yml")).unwrap();

        let backend = DockerComposeBackend;
        let result = backend.find_config(temp.path());
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("docker-compose.yml"));
    }

    #[test]
    fn test_find_config_compose_yml() {
        use std::fs::File;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        File::create(temp.path().join("compose.yml")).unwrap();

        let backend = DockerComposeBackend;
        let result = backend.find_config(temp.path());
        assert!(result.is_some());
    }

    #[test]
    fn test_find_config_not_found() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let backend = DockerComposeBackend;
        let result = backend.find_config(temp.path());
        assert!(result.is_none());
    }
}
