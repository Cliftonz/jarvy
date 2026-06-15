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
//! - **gem**: Ruby gems via `[gem]` section (future)
//! - **go**: Go binaries via `[go]` section (future)
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
mod npm;
mod nuget;
mod pip;

pub use common::PackageError;
#[allow(unused_imports)] // PackagesConfig retained for public lib re-export
pub use config::{
    CARGO_KNOBS, CargoConfig, NPM_KNOBS, NUGET_KNOBS, NpmConfig, NugetConfig, PIP_KNOBS,
    PackagesConfig, PackagesConfigRef, PipConfig,
};

use cargo_pkg::CargoHandler;
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

    Ok(())
}
