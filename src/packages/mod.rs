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
pub use config::{CargoConfig, NpmConfig, NugetConfig, PackagesConfig, PipConfig};

use cargo_pkg::CargoHandler;
use npm::NpmHandler;
use nuget::NugetHandler;
use pip::PipHandler;
use std::path::Path;

/// Install all configured packages for a project
pub fn install_packages(config: &PackagesConfig, project_dir: &Path) -> Result<(), PackageError> {
    // Install npm packages
    if let Some(ref npm_config) = config.npm {
        println!("\n  Installing npm packages...");
        let handler = NpmHandler::new(npm_config.clone(), project_dir.to_path_buf());
        if let Err(e) = handler.install() {
            eprintln!("  Warning: npm install failed: {}", e);
        }
    }

    // Install pip packages
    if let Some(ref pip_config) = config.pip {
        println!("\n  Installing pip packages...");
        let handler = PipHandler::new(pip_config.clone(), project_dir.to_path_buf());
        if let Err(e) = handler.install() {
            eprintln!("  Warning: pip install failed: {}", e);
        }
    }

    // Install cargo packages
    if let Some(ref cargo_config) = config.cargo {
        println!("\n  Installing cargo binaries...");
        let handler = CargoHandler::new(cargo_config.clone());
        if let Err(e) = handler.install() {
            eprintln!("  Warning: cargo install failed: {}", e);
        }
    }

    // Install .NET global tools
    if let Some(ref nuget_config) = config.nuget {
        println!("\n  Installing .NET global tools...");
        let handler = NugetHandler::new(nuget_config.clone());
        if let Err(e) = handler.install() {
            eprintln!("  Warning: nuget install failed: {}", e);
        }
    }

    Ok(())
}
