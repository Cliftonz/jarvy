//! .NET global tool installation handler
//!
//! Installs .NET global tools via `dotnet tool install -g <name>`. NuGet is
//! the .NET package ecosystem; "tools" here are CLI binaries published as
//! NuGet packages (e.g. `dotnet-ef`, `csharpier`, `dotnet-outdated-tool`).
//!
//! This handler does NOT manage project-level NuGet PackageReferences —
//! those belong in the project's `.csproj`/`Directory.Packages.props` and are
//! restored by `dotnet restore` during build, not by `jarvy setup`.

use super::common::{
    PackageError, command_exists, run_package_command, validate_package_name,
    validate_package_version,
};
use super::config::{NugetConfig, PackageSpec};

/// Handler for .NET global tool installation
pub struct NugetHandler {
    config: NugetConfig,
}

impl NugetHandler {
    /// Create a new nuget handler
    pub fn new(config: NugetConfig) -> Self {
        Self { config }
    }

    /// Install all configured global tools
    pub fn install(&self) -> Result<(), PackageError> {
        if !command_exists("dotnet") {
            return Err(PackageError::PackageManagerNotInstalled(
                "dotnet".to_string(),
            ));
        }

        if self.config.packages.is_empty() {
            println!("    No NuGet global tools configured");
            return Ok(());
        }

        // `dotnet tool update -g` is fully machine-global; cwd is
        // semantically irrelevant. Resolve once at the top of the loop
        // so we don't pay a getcwd(3) syscall + PathBuf allocation per
        // tool — and so a deleted-cwd condition doesn't surface as a
        // mid-loop install failure unrelated to the package itself.
        let current_dir = std::env::current_dir().map_err(PackageError::Io)?;

        for (name, spec) in &self.config.packages {
            if spec.is_optional() {
                continue;
            }
            if let Err(e) = self.install_tool(name, spec, &current_dir) {
                tracing::warn!(
                    event = "package.install_failed",
                    ecosystem = "nuget",
                    package = %name,
                    error = %e,
                );
                eprintln!("    Warning: Failed to install {}: {}", name, e);
            }
        }

        Ok(())
    }

    /// Install a single .NET global tool. Treats already-installed as success
    /// so re-runs are idempotent — `dotnet tool install -g` exits non-zero
    /// when the tool is already present, but `dotnet tool update -g` is the
    /// idempotent path we actually want.
    fn install_tool(
        &self,
        name: &str,
        spec: &PackageSpec,
        working_dir: &std::path::Path,
    ) -> Result<(), PackageError> {
        validate_package_name(name, "[nuget]")?;
        validate_package_version(spec.version(), "[nuget]")?;

        println!("    Installing {}...", name);
        // Emit per-package events through `tracing` directly, but only
        // when the user has opted into telemetry. `observability::
        // telemetry_gate` is populated by `telemetry::init` at startup
        // and gives lib-side modules a way to honor the consent gate without
        // reaching the bin-only `crate::telemetry::is_enabled()`. The
        // gate prevents `package.*` events from leaking to a
        // user-configured OTLP endpoint when `telemetry.enabled =
        // false` — the prior round emitted unconditionally and broke
        // the documented consent contract.
        let telemetry_on = crate::observability::telemetry_gate::is_enabled();
        if telemetry_on {
            tracing::info!(
                event = "package.requested",
                ecosystem = "nuget",
                package = %name,
                version = %spec.version(),
                source = "config",
                platform = std::env::consts::OS,
            );
        }
        let started = std::time::Instant::now();

        let args = build_install_args(name, spec.version());
        let _pkg_span = tracing::info_span!(
            "package",
            ecosystem = "nuget",
            name = %name,
            version = %spec.version(),
        )
        .entered();
        match run_package_command("dotnet", &args, working_dir) {
            Ok(()) => {
                if telemetry_on {
                    tracing::info!(
                        event = "package.installed",
                        ecosystem = "nuget",
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
                // Demoted from `error!` to `warn!`: per-package
                // failures are advisory (setup continues). The whole
                // ecosystem failing is the actually-pager-worthy
                // event and that one stays `error!` in `mod.rs`.
                if telemetry_on {
                    tracing::warn!(
                        event = "package.failed",
                        ecosystem = "nuget",
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

/// Build the argv passed to `dotnet`. Extracted so the contract — `tool
/// update -g <name>` (idempotent — `install -g` errors when present, so
/// we use `update` instead), with `--version <ver>` only when not
/// "latest" — can be pinned by a unit test independently of subprocess
/// dispatch.
pub(crate) fn build_install_args<'a>(name: &'a str, version: &'a str) -> Vec<&'a str> {
    let mut args: Vec<&str> = Vec::with_capacity(6);
    args.extend_from_slice(&["tool", "update", "-g", name]);
    if version != "latest" {
        args.push("--version");
        args.push(version);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn nuget_handler_empty() {
        let config = NugetConfig::default();
        let handler = NugetHandler::new(config);
        assert!(handler.config.packages.is_empty());
    }

    #[test]
    fn nuget_handler_holds_packages() {
        let mut packages = HashMap::new();
        packages.insert(
            "dotnet-ef".to_string(),
            PackageSpec::Version("latest".to_string()),
        );
        packages.insert(
            "csharpier".to_string(),
            PackageSpec::Version("0.30.0".to_string()),
        );
        let config = NugetConfig { packages };
        let handler = NugetHandler::new(config);
        assert_eq!(handler.config.packages.len(), 2);
    }

    /// Pin the argv contract — flipping `update` → `install` or dropping
    /// `-g` would change semantics catastrophically (loses idempotency,
    /// or installs per-project instead of machine-global). This test
    /// makes those regressions impossible to ship silently.
    #[test]
    fn build_install_args_table() {
        let cases = [
            (
                "dotnet-ef",
                "latest",
                vec!["tool", "update", "-g", "dotnet-ef"],
            ),
            (
                "csharpier",
                "0.30.0",
                vec!["tool", "update", "-g", "csharpier", "--version", "0.30.0"],
            ),
            (
                "dotnet-aspnet-codegenerator",
                "8.0.0",
                vec![
                    "tool",
                    "update",
                    "-g",
                    "dotnet-aspnet-codegenerator",
                    "--version",
                    "8.0.0",
                ],
            ),
        ];
        for (name, version, expected) in cases {
            let actual = build_install_args(name, version);
            assert_eq!(actual, expected, "argv mismatch for {} = {}", name, version);
            // Invariants the argv must always satisfy
            assert_eq!(actual[0], "tool", "first arg must be `tool`");
            assert_eq!(actual[1], "update", "must use `update` for idempotency");
            assert_eq!(actual[2], "-g", "must be global install");
            assert_ne!(actual[1], "install", "`install` errors when present");
        }
    }

    /// `NugetHandler::install` must reject a flag-like name via the
    /// shared validator BEFORE it reaches `dotnet`. Tests the error
    /// variant explicitly, not the tautology that `Result` is a result.
    #[test]
    fn nuget_rejects_flag_like_tool_names() {
        // Build the args directly so we exercise the validator in
        // `install_tool` without depending on whether `dotnet` is
        // installed on the test host.
        let mut packages = HashMap::new();
        packages.insert(
            "--source".to_string(),
            PackageSpec::Version("latest".to_string()),
        );
        let config = NugetConfig { packages };
        let handler = NugetHandler::new(config);
        // If dotnet is not on PATH, the outer guard fires first
        // (PackageManagerNotInstalled). Otherwise the per-tool
        // validation rejects with RefusedUnsafeSpec. Both prove the
        // attack surface is closed.
        let result = handler.install();
        match &result {
            Ok(()) | Err(PackageError::PackageManagerNotInstalled(_)) => {
                // Outer guard fired (no dotnet on PATH). The per-tool
                // path is tested directly below.
            }
            Err(other) => panic!("unexpected outer error: {other:?}"),
        }
        // Per-tool guard, exercised directly:
        let spec = PackageSpec::Version("latest".to_string());
        let err = handler
            .install_tool("--source", &spec, std::path::Path::new("."))
            .expect_err("flag-like name must be refused");
        assert!(
            matches!(err, PackageError::RefusedUnsafeSpec(_, _)),
            "expected RefusedUnsafeSpec, got {err:?}"
        );
    }
}
