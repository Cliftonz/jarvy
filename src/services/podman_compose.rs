//! Podman Compose service backend.
//!
//! Mirrors [`super::docker_compose::DockerComposeBackend`] but talks to
//! `podman` instead of `docker`. Podman ships two compose-compatible
//! entry points: the modern `podman compose` subcommand (Podman v4+
//! wraps docker-compose v2 or podman-compose) and the standalone
//! `podman-compose` PyPI package. Detection prefers the modern
//! subcommand, falling back to standalone.
//!
//! Config files are the same `docker-compose.yml` / `compose.yml`
//! set — Podman deliberately reads Docker's format. The disambiguator
//! between Docker Compose and Podman Compose in
//! `services::detect_backend` is which CLI is on PATH, not which file
//! is present.

use super::preflight::{podman_daemon_hint, probe_container_daemon, DaemonState};
use super::{
    command_exists, run_command, ServiceBackend, ServiceBackendOps, ServiceError, ServiceResult,
    ServiceStatus,
};
use crate::observability::telemetry_gate;
use crate::telemetry;
use std::path::{Path, PathBuf};

/// Pure fallback resolver — pick a compose entry point given the two
/// runtime-probe booleans. Extracted for testability; the wrapper
/// `PodmanComposeBackend::compose_command` supplies live probes.
/// `subcmd` (Podman v4+ `podman compose`) always wins when available.
/// Returns None when neither is present so tests can assert the
/// missing-both branch explicitly.
pub(super) fn compose_command_from(
    subcmd: bool,
    standalone: bool,
) -> Option<(&'static str, Vec<&'static str>)> {
    if subcmd {
        Some(("podman", vec!["compose"]))
    } else if standalone {
        Some(("podman-compose", vec![]))
    } else {
        None
    }
}

/// Podman Compose backend implementation.
pub struct PodmanComposeBackend;

impl PodmanComposeBackend {
    /// `podman compose` subcommand available (modern podman v4+).
    fn has_podman_compose_subcommand() -> bool {
        std::process::Command::new("podman")
            .args(["compose", "version"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Standalone `podman-compose` binary available.
    fn has_standalone_podman_compose() -> bool {
        command_exists("podman-compose")
    }

    /// Return `(cmd, prefix_args)` for the best available compose entry point.
    /// Memoized in a process-lifetime `OnceLock` — earlier code re-probed
    /// `podman compose version` on every `start` / `stop` / `status`
    /// invocation (3-4 subprocesses per compose op on Windows Podman;
    /// perf F3). Cache miss cost is a single `podman compose version` +
    /// `which podman-compose` at process startup; every subsequent call
    /// is a single load from the atomic.
    fn compose_command() -> (&'static str, Vec<&'static str>) {
        static CACHED: std::sync::OnceLock<(&'static str, Vec<&'static str>)> =
            std::sync::OnceLock::new();
        CACHED
            .get_or_init(|| {
                compose_command_from(
                    Self::has_podman_compose_subcommand(),
                    Self::has_standalone_podman_compose(),
                )
                .unwrap_or(("podman-compose", vec![]))
            })
            .clone()
    }

    /// Bounded label for `services.backend_selected.podman_variant`.
    /// Answers the product question "should we drop standalone
    /// podman-compose support in v0.7?" — needs actual usage split
    /// between the built-in subcommand (podman v4+) and the standalone
    /// PyPI package.
    pub(super) fn compose_variant_label(&self) -> &'static str {
        if !crate::tools::common::command_on_path("podman") {
            "n/a"
        } else if Self::has_podman_compose_subcommand() {
            "podman_compose_builtin"
        } else if Self::has_standalone_podman_compose() {
            "podman_compose_standalone"
        } else {
            "podman_compose_missing"
        }
    }
}

impl ServiceBackendOps for PodmanComposeBackend {
    fn is_installed(&self) -> bool {
        command_exists("podman")
            && (Self::has_podman_compose_subcommand() || Self::has_standalone_podman_compose())
    }

    fn find_config(&self, dir: &Path) -> Option<PathBuf> {
        for filename in ServiceBackend::PodmanCompose.config_files() {
            let path = dir.join(filename);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    fn check_daemon(&self) -> Result<(), ServiceError> {
        let started = std::time::Instant::now();
        let state = probe_container_daemon("podman");
        let duration_ms = started.elapsed().as_millis() as u64;
        if telemetry_gate::is_enabled() {
            // Info-level (was debug) — see docker_compose::check_daemon
            // for the reasoning: this is the per-invocation adoption
            // signal, and default prod log filters drop debug.
            tracing::info!(
                event = "services.daemon_check",
                backend = "podman",
                state = ?state,
                duration_ms,
                "podman daemon preflight"
            );
        }
        match state {
            DaemonState::Running => Ok(()),
            DaemonState::Down | DaemonState::Missing | DaemonState::Timeout => {
                let (hint, hint_kind) = podman_daemon_hint();
                if telemetry_gate::is_enabled() {
                    if state == DaemonState::Timeout {
                        tracing::warn!(
                            event = "services.daemon_probe_timeout",
                            backend = "podman",
                            timeout_ms = super::preflight::PROBE_TIMEOUT.as_millis() as u64,
                            duration_ms,
                            "podman daemon probe hit hard timeout, treated as down"
                        );
                    }
                    tracing::warn!(
                        event = "services.daemon_down",
                        backend = "podman",
                        state = ?state,
                        hint_kind,
                        duration_ms,
                        "podman daemon is not reachable"
                    );
                }
                Err(ServiceError::DaemonNotRunning {
                    backend: ServiceBackend::PodmanCompose,
                    hint,
                })
            }
        }
    }

    fn start(&self, config_path: &Path, detach: bool) -> Result<ServiceResult, ServiceError> {
        if !self.is_installed() {
            return Err(ServiceError::BackendNotInstalled(
                ServiceBackend::PodmanCompose,
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
            telemetry::service_operation("podman-compose", "start", true);
            Ok(ServiceResult {
                success: true,
                message: "Services started successfully".to_string(),
                backend: ServiceBackend::PodmanCompose,
            })
        } else {
            telemetry::service_operation("podman-compose", "start", false);
            Err(ServiceError::CommandFailed {
                backend: ServiceBackend::PodmanCompose,
                operation: "start",
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            })
        }
    }

    fn stop(&self, config_path: &Path) -> Result<ServiceResult, ServiceError> {
        if !self.is_installed() {
            return Err(ServiceError::BackendNotInstalled(
                ServiceBackend::PodmanCompose,
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
            telemetry::service_operation("podman-compose", "stop", true);
            Ok(ServiceResult {
                success: true,
                message: "Services stopped successfully".to_string(),
                backend: ServiceBackend::PodmanCompose,
            })
        } else {
            telemetry::service_operation("podman-compose", "stop", false);
            Err(ServiceError::CommandFailed {
                backend: ServiceBackend::PodmanCompose,
                operation: "stop",
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            })
        }
    }

    fn status(&self, config_path: &Path) -> Result<ServiceStatus, ServiceError> {
        if !self.is_installed() {
            return Ok(ServiceStatus {
                backend: ServiceBackend::PodmanCompose,
                installed: false,
                running: false,
                details: "Podman Compose is not installed".to_string(),
            });
        }
        match self.check_daemon() {
            Ok(()) => {}
            Err(ServiceError::DaemonNotRunning { hint, .. }) => {
                return Ok(ServiceStatus {
                    backend: ServiceBackend::PodmanCompose,
                    installed: true,
                    running: false,
                    details: format!("Podman daemon is not running. {hint}"),
                });
            }
            Err(other) => {
                if telemetry_gate::is_enabled() {
                    tracing::warn!(
                        event = "services.status_daemon_check_swallowed",
                        backend = "podman",
                        "unexpected error from check_daemon; reporting unknown state"
                    );
                }
                return Ok(ServiceStatus {
                    backend: ServiceBackend::PodmanCompose,
                    installed: true,
                    running: false,
                    details: format!("Podman daemon state unknown: {other}"),
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

        let lines: Vec<&str> = stdout.lines().collect();
        let running = lines.len() > 1;

        Ok(ServiceStatus {
            backend: ServiceBackend::PodmanCompose,
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

        let backend = PodmanComposeBackend;
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

        let backend = PodmanComposeBackend;
        let result = backend.find_config(temp.path());
        assert!(result.is_some());
    }

    #[test]
    fn test_find_config_not_found() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let backend = PodmanComposeBackend;
        let result = backend.find_config(temp.path());
        assert!(result.is_none());
    }

    /// Compose entry-point fallback — the subcommand-vs-standalone
    /// decision is load-bearing (Podman v3 users only have the
    /// standalone binary; v4+ users get both; some distros ship only
    /// one). Pure fn so we can table-drive without touching PATH.
    #[test]
    fn compose_command_fallback_is_correct() {
        assert_eq!(
            compose_command_from(true, false),
            Some(("podman", vec!["compose"])),
            "subcmd only -> podman compose"
        );
        assert_eq!(
            compose_command_from(false, true),
            Some(("podman-compose", vec![])),
            "standalone only -> podman-compose"
        );
        assert_eq!(
            compose_command_from(true, true),
            Some(("podman", vec!["compose"])),
            "both -> subcmd wins"
        );
        assert_eq!(
            compose_command_from(false, false),
            None,
            "neither -> None so caller can fall back / refuse"
        );
    }
}
