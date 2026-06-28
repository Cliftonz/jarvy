//! Configuration types for package management
//!
//! Defines the configuration structures for the `[npm]`, `[pip]`, `[cargo]`,
//! `[nuget]`, `[gem]`, and `[go]` sections in jarvy.toml. All six have
//! shipping install handlers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level packages configuration containing all package manager configs.
///
/// Retained for the public lib re-export (`jarvy::PackagesConfig`).
/// Internal call sites should prefer `PackagesConfigRef<'_>` plus
/// `Config::packages_ref()`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[allow(dead_code)] // Public-lib API surface
pub struct PackagesConfig {
    /// npm/yarn/pnpm package configuration
    pub npm: Option<NpmConfig>,
    /// pip/uv package configuration
    pub pip: Option<PipConfig>,
    /// cargo binary installation configuration
    pub cargo: Option<CargoConfig>,
    /// .NET global tool installation configuration
    pub nuget: Option<NugetConfig>,
    /// Ruby gem installation configuration
    pub gem: Option<GemConfig>,
    /// Go binary installation configuration
    pub go: Option<GoConfig>,
}

/// Borrowed view of every `[npm]/[pip]/[cargo]/[nuget]` block on a
/// `Config`, plus the trust gate that decides whether remote-fetched
/// configs may install packages. Use this when you only need to *read*
/// the package sections — typically `install_packages` and
/// `run_packages_phase` in setup. Constructing this is zero-allocation.
#[derive(Debug, Clone, Copy)]
pub struct PackagesConfigRef<'a> {
    pub npm: Option<&'a NpmConfig>,
    pub pip: Option<&'a PipConfig>,
    pub cargo: Option<&'a CargoConfig>,
    pub nuget: Option<&'a NugetConfig>,
    pub gem: Option<&'a GemConfig>,
    pub go: Option<&'a GoConfig>,
    /// Where the parent `Config` came from. `Remote` configs are
    /// refused at install time unless `allow_remote_packages` is true.
    pub origin: crate::ai_hooks::ConfigOrigin,
    /// `[packages] allow_remote` opt-in. False by default — a remote
    /// config CANNOT install `[npm]/[pip]/[cargo]/[nuget]/[gem]/[go]`
    /// entries without the user explicitly setting this true.
    pub allow_remote_packages: bool,
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

// Canonical knob lists for `validate_package_section` — these are the
// keys inside a `[npm]/[pip]/[cargo]/[nuget]` table that are config
// options, NOT package names. Adding a new field to a `*Config` struct
// without adding it here will be rejected at compile time by the
// destructure tests below — the validator would otherwise refuse the
// new knob as a hostile package name.

/// Non-package keys in `[npm]`.
pub const NPM_KNOBS: &[&str] = &["package_manager", "from_lockfile", "install_dev"];

/// Non-package keys in `[pip]`.
pub const PIP_KNOBS: &[&str] = &[
    "venv",
    "create_venv",
    "from_lockfile",
    "lockfile",
    "activate_hint",
    "system_site_packages",
    "python_version",
];

/// Non-package keys in `[cargo]`.
pub const CARGO_KNOBS: &[&str] = &["locked"];

/// Non-package keys in `[nuget]`. Empty today — adding a knob to
/// `NugetConfig` requires updating this slice too (the destructure
/// test below catches the miss).
pub const NUGET_KNOBS: &[&str] = &[];

/// Non-package keys in `[gem]`. Empty today.
pub const GEM_KNOBS: &[&str] = &[];

/// Non-package keys in `[go]`. Empty today.
pub const GO_KNOBS: &[&str] = &[];

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

/// gem/bundler Ruby package configuration.
///
/// Installs via `gem install <name> [-v <version>]` against the user's
/// active ruby. Project-level `Gemfile.lock` workflows are out of scope;
/// run `bundle install` from project bootstrap, not `jarvy setup`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GemConfig {
    /// Individual gems with versions
    #[serde(flatten)]
    pub packages: HashMap<String, PackageSpec>,
}

/// Go binary installation configuration.
///
/// Installs via `go install <module>@<version>` to the user's `GOBIN`.
/// Module paths are full import paths (e.g.
/// `github.com/golangci/golangci-lint/cmd/golangci-lint`).
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

    // ===== knob-list drift guards =====
    //
    // Mirror the `TOP_LEVEL_SECTIONS` destructure pattern in `config.rs`.
    // If anyone adds a non-`#[serde(flatten)]` field to a `*Config`
    // without updating the matching `*_KNOBS` slice, these tests fail
    // to compile because the destructure is missing the new binding.
    // The fix points the developer at both the struct AND the slice
    // in one step, preventing the "validator rejects legitimate knob
    // as hostile package name" class of bug.

    /// `PackagesConfigRef` and `PackagesConfig` must carry the same six
    /// ecosystems. If anyone adds a seventh, both destructures fail to
    /// compile until they're updated together — the owned-struct one
    /// because it gains a new field, the ref-struct because it doesn't.
    /// Stops "added a ref field but forgot to wire it from
    /// `Config::packages_ref()`" regressions.
    #[test]
    fn packages_ref_field_set_matches_owned_ecosystems() {
        fn _enforce_owned(c: PackagesConfig) {
            let PackagesConfig {
                npm: _,
                pip: _,
                cargo: _,
                nuget: _,
                gem: _,
                go: _,
            } = c;
        }
        fn _enforce_ref(r: PackagesConfigRef<'_>) {
            let PackagesConfigRef {
                npm: _,
                pip: _,
                cargo: _,
                nuget: _,
                gem: _,
                go: _,
                origin: _,
                allow_remote_packages: _,
            } = r;
        }
    }

    #[test]
    fn npm_knobs_match_npm_config_fields() {
        fn _enforce(c: NpmConfig) {
            let NpmConfig {
                packages: _,
                package_manager: _,
                from_lockfile: _,
                install_dev: _,
            } = c;
        }
        for k in ["package_manager", "from_lockfile", "install_dev"] {
            assert!(
                NPM_KNOBS.contains(&k),
                "NpmConfig has knob `{}` but NPM_KNOBS is missing it",
                k
            );
        }
    }

    #[test]
    fn pip_knobs_match_pip_config_fields() {
        fn _enforce(c: PipConfig) {
            let PipConfig {
                packages: _,
                venv: _,
                create_venv: _,
                from_lockfile: _,
                lockfile: _,
                activate_hint: _,
                system_site_packages: _,
                python_version: _,
            } = c;
        }
        for k in [
            "venv",
            "create_venv",
            "from_lockfile",
            "lockfile",
            "activate_hint",
            "system_site_packages",
            "python_version",
        ] {
            assert!(
                PIP_KNOBS.contains(&k),
                "PipConfig has knob `{}` but PIP_KNOBS is missing it",
                k
            );
        }
    }

    #[test]
    fn cargo_knobs_match_cargo_config_fields() {
        fn _enforce(c: CargoConfig) {
            let CargoConfig {
                packages: _,
                locked: _,
            } = c;
        }
        assert!(
            CARGO_KNOBS.contains(&"locked"),
            "CargoConfig has knob `locked` but CARGO_KNOBS is missing it"
        );
    }

    #[test]
    fn nuget_knobs_match_nuget_config_fields() {
        fn _enforce(c: NugetConfig) {
            let NugetConfig { packages: _ } = c;
        }
        // NUGET_KNOBS is intentionally empty today. When/if a knob is
        // added, both the destructure above AND NUGET_KNOBS must be
        // updated together.
        assert!(NUGET_KNOBS.is_empty(), "NugetConfig has no knobs yet");
    }

    #[test]
    fn gem_knobs_match_gem_config_fields() {
        fn _enforce(c: GemConfig) {
            let GemConfig { packages: _ } = c;
        }
        assert!(GEM_KNOBS.is_empty(), "GemConfig has no knobs yet");
    }

    #[test]
    fn go_knobs_match_go_config_fields() {
        fn _enforce(c: GoConfig) {
            let GoConfig { packages: _ } = c;
        }
        assert!(GO_KNOBS.is_empty(), "GoConfig has no knobs yet");
    }

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
