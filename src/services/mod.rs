//! Service Management module
//!
//! This module provides functionality for:
//! - Detecting and managing project services (docker-compose, tilt)
//! - Starting, stopping, and checking status of services
//! - Integration with jarvy setup flow

#![allow(dead_code)] // Public API for service management

mod docker_compose;
mod podman_compose;
mod preflight;
mod tilt;

pub use docker_compose::DockerComposeBackend;
pub use podman_compose::PodmanComposeBackend;
pub use tilt::TiltBackend;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// Service backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceBackend {
    /// Docker Compose (docker-compose.yml or compose.yml)
    DockerCompose,
    /// Podman Compose (falls back to Docker Compose config files)
    PodmanCompose,
    /// Tilt (Tiltfile)
    Tilt,
}

impl ServiceBackend {
    /// Returns the human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::DockerCompose => "Docker Compose",
            Self::PodmanCompose => "Podman Compose",
            Self::Tilt => "Tilt",
        }
    }

    /// Returns the default config file name(s)
    pub fn config_files(&self) -> &'static [&'static str] {
        match self {
            Self::DockerCompose | Self::PodmanCompose => &[
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
            Self::PodmanCompose => "podman",
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
    /// Backend binary is installed but its daemon is not reachable.
    /// `hint` is a platform-aware, user-actionable next step (e.g.
    /// "Start Docker Desktop" / "colima start" / "sudo systemctl start docker").
    DaemonNotRunning {
        backend: ServiceBackend,
        hint: String,
    },
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
            Self::DaemonNotRunning { backend, hint } => {
                write!(f, "{} daemon is not running. {}", backend, hint)
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

    /// Preflight check: verify the backend's daemon is reachable.
    ///
    /// Default implementation is a no-op (`Ok(())`). Backends whose CLI
    /// tool talks to a long-running daemon (Docker, Podman rootful,
    /// Colima-backed Docker) should override to probe the daemon with
    /// a low-overhead command like `docker info` before attempting
    /// `start` / `status`. Returning `Err(DaemonNotRunning { hint })`
    /// short-circuits `start()` / `status()` with an actionable
    /// message instead of a raw `compose up` failure.
    fn check_daemon(&self) -> Result<(), ServiceError> {
        Ok(())
    }

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

/// Detect which service backend is available in the given directory.
///
/// Priority: Docker Compose > Podman Compose > Tilt.
///
/// Docker and Podman share config files (`docker-compose.yml` / `compose.yml`),
/// so the disambiguator is which CLI is installed. If `docker` is on PATH
/// we pick Docker Compose; if only `podman` is on PATH we pick Podman
/// Compose. Users who want to force Podman despite Docker also being
/// installed can set `[services] compose_file` with a `podman-compose`
/// variant or extend the config in a follow-up.
pub fn detect_backend(dir: &Path) -> Option<(ServiceBackend, PathBuf)> {
    let docker = DockerComposeBackend;
    if let Some(path) = docker.find_config(dir) {
        let podman = PodmanComposeBackend;
        let docker_installed = docker.is_installed();
        let podman_installed = podman.is_installed();
        let picked = if docker_installed {
            ServiceBackend::DockerCompose
        } else if podman_installed {
            ServiceBackend::PodmanCompose
        } else {
            ServiceBackend::DockerCompose
        };
        emit_backend_selected(
            picked,
            docker_installed,
            podman_installed,
            podman.compose_variant_label(),
            "detect",
        );
        return Some((picked, path));
    }

    let tilt = TiltBackend;
    if let Some(path) = tilt.find_config(dir) {
        emit_backend_selected(ServiceBackend::Tilt, false, false, "n/a", "detect");
        return Some((ServiceBackend::Tilt, path));
    }

    None
}

/// Emit `services.backend_selected` — the per-invocation attribution
/// event for "which backend does this project actually run against?"
/// Enables the Podman-vs-Docker adoption funnel (`podman_selected /
/// total_selected`) that `services.daemon_check` alone couldn't
/// answer (the probe fires per CLI, not per compose op).
fn emit_backend_selected(
    picked: ServiceBackend,
    docker_installed: bool,
    podman_installed: bool,
    podman_variant: &'static str,
    source: &'static str,
) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    let backend_label = match picked {
        ServiceBackend::DockerCompose => "docker",
        ServiceBackend::PodmanCompose => "podman",
        ServiceBackend::Tilt => "tilt",
    };
    tracing::info!(
        event = "services.backend_selected",
        backend = backend_label,
        docker_installed,
        podman_installed,
        podman_variant,
        source,
        "backend picked for services phase"
    );
}

/// Resolve a `[services.compose_file] | [services.tiltfile]` config path
/// against the project root and refuse anything that would escape it.
/// A hostile project `jarvy.toml` setting `compose_file = "../../etc/x.yml"`
/// or `compose_file = "/tmp/evil/compose.yml"` is dropped here so docker
/// compose / tilt never gets handed an attacker-staged file containing
/// `volumes: ["/:/host"]` or a privileged service definition.
fn resolve_within_project(dir: &Path, raw: &Path, kind: &'static str) -> Option<PathBuf> {
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        dir.join(raw)
    };
    let canonical_candidate = std::fs::canonicalize(&candidate).ok()?;
    let canonical_root = std::fs::canonicalize(dir).ok()?;
    if !canonical_candidate.starts_with(&canonical_root) {
        tracing::warn!(
            event = "services.refused_escape",
            kind = %kind,
            path = %canonical_candidate.display(),
            root = %canonical_root.display(),
            "refused [services] path that escapes project root"
        );
        return None;
    }
    Some(canonical_candidate)
}

/// Detect which service backend is available, with config override support
pub fn detect_backend_with_config(
    dir: &Path,
    compose_file: Option<&Path>,
    tilt_file: Option<&Path>,
) -> Option<(ServiceBackend, PathBuf)> {
    // If compose_file is explicitly set, use it (containment-checked).
    // Docker wins when both are installed; podman only if docker is absent.
    if let Some(compose) = compose_file
        && let Some(path) = resolve_within_project(dir, compose, "compose_file")
        && path.exists()
    {
        let docker = DockerComposeBackend;
        let podman = PodmanComposeBackend;
        let docker_installed = docker.is_installed();
        let podman_installed = podman.is_installed();
        let picked = if docker_installed {
            ServiceBackend::DockerCompose
        } else if podman_installed {
            ServiceBackend::PodmanCompose
        } else {
            ServiceBackend::DockerCompose
        };
        emit_backend_selected(
            picked,
            docker_installed,
            podman_installed,
            podman.compose_variant_label(),
            "compose_file_override",
        );
        return Some((picked, path));
    }

    // If tilt_file is explicitly set, use it (containment-checked).
    if let Some(tilt) = tilt_file
        && let Some(path) = resolve_within_project(dir, tilt, "tiltfile")
        && path.exists()
    {
        return Some((ServiceBackend::Tilt, path));
    }

    // Fall back to auto-detection
    detect_backend(dir)
}

/// Get the appropriate backend implementation
pub fn get_backend(backend: ServiceBackend) -> Box<dyn ServiceBackendOps> {
    match backend {
        ServiceBackend::DockerCompose => Box::new(DockerComposeBackend),
        ServiceBackend::PodmanCompose => Box::new(PodmanComposeBackend),
        ServiceBackend::Tilt => Box::new(TiltBackend),
    }
}

/// Check if a command exists in PATH. Thin alias over the shared
/// `tools::common::command_on_path` — kept as a module-private
/// name so call sites in this crate can `use super::command_exists`
/// without threading a new import through six files. Fixes an
/// earlier bug where this shelled to `which` unconditionally, which
/// silently returned `false` on every Windows box and broke the
/// Podman/Docker backend detection this diff was meant to support.
fn command_exists(cmd: &str) -> bool {
    crate::tools::common::command_on_path(cmd)
}

/// Run a command and capture output.
///
/// Wrapped in a `tracing::info_span!("subprocess.exec", ...)` so support
/// can see what command the docker-compose / tilt path is running when
/// `jarvy setup` stalls on the services phase. Without this, a hung
/// `docker compose up -d` is invisible in `~/.jarvy/logs/jarvy.log`
/// (round-2 obs F16).
fn run_command(cmd: &str, args: &[&str], working_dir: &Path) -> Result<Output, std::io::Error> {
    let span = tracing::info_span!(
        "subprocess.exec",
        cmd = %cmd,
        args_count = args.len(),
        cwd = %working_dir.display(),
    );
    let _g = span.enter();
    let start = std::time::Instant::now();
    let result = Command::new(cmd)
        .args(args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let duration_ms = start.elapsed().as_millis() as u64;
    match &result {
        Ok(out) => tracing::debug!(
            event = "subprocess.completed",
            exit_code = out.status.code().unwrap_or(-1),
            duration_ms,
            "subprocess finished"
        ),
        Err(e) => tracing::warn!(
            event = "subprocess.failed",
            error = %e,
            duration_ms,
            "subprocess spawn failed"
        ),
    }
    result
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
