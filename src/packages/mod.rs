//! Language package dependency management
//!
//! This module provides support for installing language-specific packages
//! (npm, pip, cargo, gem, go) alongside CLI tools, enabling complete
//! development environment reproducibility from a single `jarvy.toml` configuration.
//!
//! # Supported Package Managers
//!
//! - **npm/yarn/pnpm**: Node.js packages via `[npm]` section
//! - **pip/uv**: Python packages via `[pip]` section with virtual environment support
//! - **cargo**: Rust binaries via `[cargo]` section
//! - **nuget**: .NET global tools via `[nuget]` section
//! - **gem**: Ruby gems via `[gem]` section
//! - **go**: Go binaries via `[go]` section
//!
//! # Example Configuration
//!
//! ```toml
//! [npm]
//! typescript = "^5.0"
//! eslint = "latest"
//! package_manager = "pnpm"
//!
//! [pip]
//! pytest = ">=7.0"
//! black = "latest"
//! venv = ".venv"
//!
//! [cargo]
//! cargo-watch = "latest"
//! cargo-nextest = "latest"
//!
//! [nuget]
//! dotnet-ef = "latest"
//! csharpier = "0.30.0"
//! ```

mod cargo_pkg;
pub mod common;
mod config;
mod gem;
mod go;
mod npm;
mod nuget;
mod pip;

pub use common::PackageError;
#[allow(unused_imports)] // PackagesConfig retained for public lib re-export
pub use config::{
    CARGO_KNOBS, CargoConfig, GEM_KNOBS, GO_KNOBS, GemConfig, GoConfig, NPM_KNOBS, NUGET_KNOBS,
    NpmConfig, NugetConfig, PIP_KNOBS, PackagesConfig, PackagesConfigRef, PipConfig,
};

use cargo_pkg::CargoHandler;
use gem::GemHandler;
use go::GoHandler;
use npm::NpmHandler;
use nuget::NugetHandler;
use pip::PipHandler;
use std::path::Path;

/// Install all configured packages for a project. Accepts a borrowed
/// view (`PackagesConfigRef`) so callers don't pay 4 deep HashMap
/// clones just to read which ecosystems are configured — the handler
/// constructors still clone what they own, but that's one clone per
/// ecosystem instead of two.
pub fn install_packages(
    config: PackagesConfigRef<'_>,
    project_dir: &Path,
) -> Result<(), PackageError> {
    // Trust gate: a remote-fetched config (`jarvy setup --from <url>`)
    // CANNOT install language-package entries without an explicit
    // opt-in (`[packages] allow_remote = true`). Mirrors the
    // `[ai_hooks] allow_custom_commands` and `[mcp_register]
    // allow_custom_servers` patterns — remote configs may NARROW
    // trust but cannot BROADEN it.
    if config.origin == crate::ai_hooks::ConfigOrigin::Remote && !config.allow_remote_packages {
        let any_configured = config.npm.is_some()
            || config.pip.is_some()
            || config.cargo.is_some()
            || config.nuget.is_some()
            || config.gem.is_some()
            || config.go.is_some();
        if any_configured {
            tracing::warn!(
                event = "packages.remote_refused",
                reason = "allow_remote_packages_not_set",
            );
            eprintln!(
                "\n  Refusing to install packages from a remote config (`jarvy setup --from <url>`).\n  \
                 Set `[packages] allow_remote = true` in the source config — or copy it locally —\n  \
                 to authorize npm/pip/cargo/nuget installations from this origin."
            );
            return Ok(());
        }
    }

    // Telemetry gate read once at the top — used by every per-ecosystem
    // branch below. Honors the user's opt-in.
    let telemetry_on = crate::observability::telemetry_gate::is_enabled();

    // Install npm packages
    if let Some(npm_config) = config.npm {
        println!("\n  Installing npm packages...");
        let handler = NpmHandler::new(npm_config.clone(), project_dir.to_path_buf());
        if let Err(e) = handler.install() {
            if telemetry_on {
                tracing::warn!(
                    event = "packages.install_failed",
                    ecosystem = "npm",
                    error_kind = e.kind(),
                    error = %e,
                );
            }
            // Compact warning — the subprocess output already streamed
            // live through `run_package_command`'s tee, and the error
            // envelope itself carries the redacted tail. Don't re-print
            // the tail through `e`'s Display because that's now the
            // full envelope and would duplicate 4KB on the user's
            // terminal.
            eprintln!("  Warning: npm install failed (see output above)");
        }
    }

    // Install pip packages
    if let Some(pip_config) = config.pip {
        println!("\n  Installing pip packages...");
        let handler = PipHandler::new(pip_config.clone(), project_dir.to_path_buf());
        if let Err(e) = handler.install() {
            if telemetry_on {
                tracing::warn!(
                    event = "packages.install_failed",
                    ecosystem = "pip",
                    error_kind = e.kind(),
                    error = %e,
                );
            }
            eprintln!("  Warning: pip install failed (see output above)");
        }
    }

    // Install cargo packages
    if let Some(cargo_config) = config.cargo {
        println!("\n  Installing cargo binaries...");
        let handler = CargoHandler::new(cargo_config.clone());
        if let Err(e) = handler.install() {
            if telemetry_on {
                tracing::warn!(
                    event = "packages.install_failed",
                    ecosystem = "cargo",
                    error_kind = e.kind(),
                    error = %e,
                );
            }
            eprintln!("  Warning: cargo install failed (see output above)");
        }
    }

    // Install .NET global tools
    if let Some(nuget_config) = config.nuget {
        println!("\n  Installing .NET global tools...");
        let handler = NugetHandler::new(nuget_config.clone());
        if let Err(e) = handler.install() {
            if telemetry_on {
                tracing::warn!(
                    event = "packages.install_failed",
                    ecosystem = "nuget",
                    error_kind = e.kind(),
                    error = %e,
                );
            }
            eprintln!("  Warning: nuget install failed (see output above)");
        }
    }

    // Install Ruby gems
    if let Some(gem_config) = config.gem {
        println!("\n  Installing Ruby gems...");
        let handler = GemHandler::new(gem_config.clone());
        if let Err(e) = handler.install() {
            if telemetry_on {
                tracing::warn!(
                    event = "packages.install_failed",
                    ecosystem = "gem",
                    error_kind = e.kind(),
                    error = %e,
                );
            }
            eprintln!("  Warning: gem install failed (see output above)");
        }
    }

    // Install Go binaries
    if let Some(go_config) = config.go {
        println!("\n  Installing Go binaries...");
        let handler = GoHandler::new(go_config.clone());
        if let Err(e) = handler.install() {
            if telemetry_on {
                tracing::warn!(
                    event = "packages.install_failed",
                    ecosystem = "go",
                    error_kind = e.kind(),
                    error = %e,
                );
            }
            eprintln!("  Warning: go install failed (see output above)");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_hooks::ConfigOrigin;

    fn ref_with_nuget_only(
        nuget: &NugetConfig,
        origin: ConfigOrigin,
        allow_remote_packages: bool,
    ) -> PackagesConfigRef<'_> {
        PackagesConfigRef {
            npm: None,
            pip: None,
            cargo: None,
            nuget: Some(nuget),
            gem: None,
            go: None,
            origin,
            allow_remote_packages,
        }
    }

    #[test]
    fn remote_config_refused_without_opt_in() {
        let nuget = NugetConfig::default();
        let config = ref_with_nuget_only(&nuget, ConfigOrigin::Remote, false);
        let tmp = tempfile::tempdir().unwrap();
        // Should return Ok (advisory, not fatal) without invoking dotnet.
        let result = install_packages(config, tmp.path());
        assert!(result.is_ok(), "trust gate should refuse silently");
    }

    #[test]
    fn remote_config_allowed_with_opt_in_proceeds() {
        // With the opt-in set, the gate passes. We don't actually
        // install anything (empty packages list), but the call should
        // not be refused at the trust-gate boundary.
        let nuget = NugetConfig::default();
        let config = ref_with_nuget_only(&nuget, ConfigOrigin::Remote, true);
        let tmp = tempfile::tempdir().unwrap();
        let result = install_packages(config, tmp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn local_config_always_allowed() {
        // Local configs ignore allow_remote_packages.
        let nuget = NugetConfig::default();
        let config = ref_with_nuget_only(&nuget, ConfigOrigin::Local, false);
        let tmp = tempfile::tempdir().unwrap();
        let result = install_packages(config, tmp.path());
        assert!(result.is_ok());
    }
}
