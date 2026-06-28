//! Go binary installation handler
//!
//! Installs Go binaries via `go install <module>@<version>`. Targets the
//! user's `GOBIN` (or `$GOPATH/bin`, or `$HOME/go/bin` fallback) — the
//! same path `go install` itself uses. Lock-file workflows (`go.mod`
//! resolution in a project tree) are intentionally out of scope.

use super::common::{
    PackageError, command_exists, run_package_command, validate_package_name,
    validate_package_version,
};
use super::config::{GoConfig, PackageSpec};

/// Handler for Go binary installation
pub struct GoHandler {
    config: GoConfig,
}

impl GoHandler {
    /// Create a new go handler
    pub fn new(config: GoConfig) -> Self {
        Self { config }
    }

    /// Install all configured go binaries
    pub fn install(&self) -> Result<(), PackageError> {
        if !command_exists("go") {
            return Err(PackageError::PackageManagerNotInstalled("go".to_string()));
        }

        if self.config.packages.is_empty() {
            println!("    No go packages configured");
            return Ok(());
        }

        // `go install` writes to GOBIN (machine-global); cwd is irrelevant.
        let current_dir = std::env::current_dir().map_err(PackageError::Io)?;

        for (name, spec) in &self.config.packages {
            if spec.is_optional() {
                continue;
            }
            if let Err(e) = self.install_binary(name, spec, &current_dir) {
                tracing::warn!(
                    event = "package.install_failed",
                    ecosystem = "go",
                    package = %name,
                    error = %e,
                );
                eprintln!("    Warning: Failed to install {}: {}", name, e);
            }
        }

        Ok(())
    }

    fn install_binary(
        &self,
        name: &str,
        spec: &PackageSpec,
        working_dir: &std::path::Path,
    ) -> Result<(), PackageError> {
        validate_package_name(name, "[go]")?;
        validate_package_version(spec.version(), "[go]")?;

        println!("    Installing {}...", name);
        let telemetry_on = crate::observability::telemetry_gate::is_enabled();
        if telemetry_on {
            tracing::info!(
                event = "package.requested",
                ecosystem = "go",
                package = %name,
                version = %spec.version(),
                source = "config",
                platform = std::env::consts::OS,
            );
        }
        let started = std::time::Instant::now();

        let module_spec = build_module_spec(name, spec.version());
        let args = ["install", module_spec.as_str()];
        let _pkg_span = tracing::info_span!(
            "package",
            ecosystem = "go",
            name = %name,
            version = %spec.version(),
        )
        .entered();
        match run_package_command("go", &args, working_dir) {
            Ok(()) => {
                if telemetry_on {
                    tracing::info!(
                        event = "package.installed",
                        ecosystem = "go",
                        package = %name,
                        version = %spec.version(),
                        source = "config",
                        duration_ms = started.elapsed().as_millis() as u64,
                        platform = std::env::consts::OS,
                    );
                }
                Ok(())
            }
            Err(e) => {
                if telemetry_on {
                    tracing::warn!(
                        event = "package.failed",
                        ecosystem = "go",
                        package = %name,
                        version = %spec.version(),
                        source = "config",
                        error_kind = e.kind(),
                        error = %e,
                        platform = std::env::consts::OS,
                    );
                }
                Err(e)
            }
        }
    }
}

/// Build the `<module>@<version>` argument that `go install` requires.
/// Go's tooling treats `@latest` and `@<semver>` as documented inputs —
/// no version implies module-graph resolution that only works inside a
/// `go.mod` tree, which is not the global-install path users want here.
pub(crate) fn build_module_spec(name: &str, version: &str) -> String {
    if version == "latest" {
        format!("{name}@latest")
    } else {
        format!("{name}@{version}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn go_handler_empty() {
        let config = GoConfig::default();
        let handler = GoHandler::new(config);
        assert!(handler.config.packages.is_empty());
    }

    #[test]
    fn go_handler_holds_packages() {
        let mut packages = HashMap::new();
        packages.insert(
            "github.com/golangci/golangci-lint/cmd/golangci-lint".to_string(),
            PackageSpec::Version("latest".to_string()),
        );
        packages.insert(
            "github.com/cosmtrek/air".to_string(),
            PackageSpec::Version("v1.49.0".to_string()),
        );
        let config = GoConfig { packages };
        let handler = GoHandler::new(config);
        assert_eq!(handler.config.packages.len(), 2);
    }

    /// `<module>@<version>` is mandatory for `go install` outside a
    /// `go.mod` tree — pin the contract.
    #[test]
    fn build_module_spec_table() {
        assert_eq!(
            build_module_spec("github.com/cosmtrek/air", "latest"),
            "github.com/cosmtrek/air@latest"
        );
        assert_eq!(
            build_module_spec("golang.org/x/tools/gopls", "v0.15.0"),
            "golang.org/x/tools/gopls@v0.15.0"
        );
    }

    /// Flag-like go module paths must be refused before they hit `go`.
    #[test]
    fn go_rejects_flag_like_names() {
        let mut packages = HashMap::new();
        packages.insert(
            "--mod".to_string(),
            PackageSpec::Version("latest".to_string()),
        );
        let config = GoConfig { packages };
        let handler = GoHandler::new(config);
        let result = handler.install();
        match &result {
            Ok(()) | Err(PackageError::PackageManagerNotInstalled(_)) => {}
            Err(other) => panic!("unexpected outer error: {other:?}"),
        }
        let spec = PackageSpec::Version("latest".to_string());
        let err = handler
            .install_binary("--mod", &spec, std::path::Path::new("."))
            .expect_err("flag-like module path must be refused");
        assert!(matches!(err, PackageError::RefusedUnsafeSpec(_, _)));
    }
}
