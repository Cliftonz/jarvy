//! Configuration types for package management
//!
//! Defines the configuration structures for the `[npm]`, `[pip]`, `[cargo]`,
//! `[nuget]`, `[gem]`, and `[go]` sections in jarvy.toml.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level packages configuration containing all package manager configs
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PackagesConfig {
    /// npm/yarn/pnpm package configuration
    pub npm: Option<NpmConfig>,
    /// pip/uv package configuration
    pub pip: Option<PipConfig>,
    /// cargo binary installation configuration
    pub cargo: Option<CargoConfig>,
    /// .NET global tool installation configuration
    pub nuget: Option<NugetConfig>,
    /// gem/bundler configuration (future)
    pub gem: Option<GemConfig>,
    /// go modules configuration (future)
    pub go: Option<GoConfig>,
}

/// Borrowed view of every `[npm]/[pip]/[cargo]/[nuget]` block on a
/// `Config`. Use this when you only need to *read* the package
/// sections — typically `install_packages` and `run_packages_phase`
/// in setup. Constructing this is zero-allocation; the previous
/// `Config::get_packages_config` path deep-cloned every HashMap.
#[derive(Debug, Clone, Copy)]
pub struct PackagesConfigRef<'a> {
    pub npm: Option<&'a NpmConfig>,
    pub pip: Option<&'a PipConfig>,
    pub cargo: Option<&'a CargoConfig>,
    pub nuget: Option<&'a NugetConfig>,
}

impl<'a> PackagesConfigRef<'a> {
    /// True if any package section is configured. Mirrors
    /// `Config::has_packages` for ref-only contexts.
    #[allow(dead_code)] // Public API for borrowed-packages-config callers
    pub fn any_configured(&self) -> bool {
        self.npm.is_some() || self.pip.is_some() || self.cargo.is_some() || self.nuget.is_some()
    }
}

/// Package specification - either a simple version string or detailed config
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PackageSpec {
    /// Simple version string (e.g., "^5.0", "latest", ">=7.0")
    Version(String),
    /// Detailed package specification with additional options
    Detailed {
        /// Version requirement
        version: String,
        /// Whether this package is optional
        #[serde(default)]
        optional: bool,
        /// Cargo features to enable
        #[serde(default)]
        features: Vec<String>,
    },
}

impl PackageSpec {
    /// Get the version string from this spec
    pub fn version(&self) -> &str {
        match self {
            PackageSpec::Version(v) => v,
            PackageSpec::Detailed { version, .. } => version,
        }
    }

    /// Check if this package is optional
    pub fn is_optional(&self) -> bool {
        match self {
            PackageSpec::Version(_) => false,
            PackageSpec::Detailed { optional, .. } => *optional,
        }
    }

    /// Get features (for cargo packages)
    pub fn features(&self) -> &[String] {
        match self {
            PackageSpec::Version(_) => &[],
            PackageSpec::Detailed { features, .. } => features,
        }
    }
}

/// npm/yarn/pnpm package configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct NpmConfig {
    /// Individual packages with versions
    #[serde(flatten)]
    pub packages: HashMap<String, PackageSpec>,

    /// Package manager to use (auto-detected from lock file if not specified)
    #[serde(default)]
    pub package_manager: Option<NpmPackageManager>,

    /// Install from existing lock file instead of individual packages
    #[serde(default)]
    pub from_lockfile: bool,

    /// Include devDependencies when installing from lock file
    #[serde(default = "default_true")]
    pub install_dev: bool,
}

/// Supported npm-compatible package managers
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NpmPackageManager {
    /// npm (Node Package Manager)
    #[default]
    Npm,
    /// Yarn
    Yarn,
    /// pnpm (Performant npm)
    Pnpm,
}

impl NpmPackageManager {
    /// Get the command name for this package manager
    pub fn command(&self) -> &'static str {
        match self {
            NpmPackageManager::Npm => "npm",
            NpmPackageManager::Yarn => "yarn",
            NpmPackageManager::Pnpm => "pnpm",
        }
    }
}

/// pip/uv Python package configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PipConfig {
    /// Individual packages with versions
    #[serde(flatten)]
    pub packages: HashMap<String, PackageSpec>,

    /// Path to virtual environment (relative to project root)
    #[serde(default)]
    pub venv: Option<String>,

    /// Create virtual environment if it doesn't exist
    #[serde(default = "default_true")]
    pub create_venv: bool,

    /// Install from existing requirements file
    #[serde(default)]
    pub from_lockfile: bool,

    /// Custom requirements file path
    #[serde(default)]
    pub lockfile: Option<String>,

    /// Show activation hint after setup
    #[serde(default = "default_true")]
    pub activate_hint: bool,

    /// Include system site-packages in virtual environment
    #[serde(default)]
    pub system_site_packages: bool,

    /// Python version to use (defaults to system python3)
    #[serde(default)]
    pub python_version: Option<String>,
}

/// cargo binary installation configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CargoConfig {
    /// Individual packages with versions
    #[serde(flatten)]
    pub packages: HashMap<String, PackageSpec>,

    /// Use --locked flag for reproducible builds
    #[serde(default)]
    pub locked: bool,
}

/// .NET global tool configuration (`dotnet tool install -g`).
///
/// "NuGet" here covers the .NET tool ecosystem — CLI binaries published as
/// NuGet packages. Project-level NuGet PackageReferences (the dependencies
/// of a `.csproj`) are NOT managed here; they belong in the project file and
/// are restored by `dotnet restore` during build.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct NugetConfig {
    /// Individual global tools with versions
    #[serde(flatten)]
    pub packages: HashMap<String, PackageSpec>,
}

/// gem/bundler Ruby package configuration (future)
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GemConfig {
    /// Individual gems with versions
    #[serde(flatten)]
    pub packages: HashMap<String, PackageSpec>,
}

/// go modules configuration (future)
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GoConfig {
    /// Individual go binaries to install
    #[serde(flatten)]
    pub packages: HashMap<String, PackageSpec>,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_npm_config() {
        let toml_str = r#"
            typescript = "^5.0"
            eslint = "latest"
        "#;

        let config: NpmConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.packages.len(), 2);
        assert!(matches!(
            config.packages.get("typescript"),
            Some(PackageSpec::Version(v)) if v == "^5.0"
        ));
    }

    #[test]
    fn test_parse_npm_with_package_manager() {
        let toml_str = r#"
            typescript = "^5.0"
            package_manager = "pnpm"
        "#;

        let config: NpmConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.package_manager, Some(NpmPackageManager::Pnpm));
    }

    #[test]
    fn test_parse_pip_with_venv() {
        let toml_str = r#"
            pytest = ">=7.0"
            venv = ".venv"
            create_venv = true
        "#;

        let config: PipConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.venv, Some(".venv".to_string()));
        assert!(config.create_venv);
        assert!(config.packages.contains_key("pytest"));
    }

    #[test]
    fn test_parse_nuget_config() {
        let toml_str = r#"
            dotnet-ef = "latest"
            csharpier = "0.30.0"
        "#;
        let config: NugetConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.packages.len(), 2);
        assert!(matches!(
            config.packages.get("dotnet-ef"),
            Some(PackageSpec::Version(v)) if v == "latest"
        ));
    }

    #[test]
    fn test_parse_cargo_config() {
        let toml_str = r#"
            cargo-watch = "latest"
            cargo-nextest = "latest"
            locked = true
        "#;

        let config: CargoConfig = toml::from_str(toml_str).unwrap();
        assert!(config.locked);
        assert_eq!(config.packages.len(), 2);
    }

    #[test]
    fn test_package_spec_detailed() {
        let toml_str = r#"
            [some-crate]
            version = "1.0.0"
            optional = true
            features = ["feature1", "feature2"]
        "#;

        #[derive(Deserialize)]
        struct Test {
            #[serde(rename = "some-crate")]
            some_crate: PackageSpec,
        }

        let test: Test = toml::from_str(toml_str).unwrap();
        assert!(test.some_crate.is_optional());
        assert_eq!(test.some_crate.features().len(), 2);
    }
}
