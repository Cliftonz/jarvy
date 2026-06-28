//! Ruby gem installation handler
//!
//! Installs Ruby gems via `gem install <name> [-v <version>]`. Targets the
//! user-active `gem` interpreter — version-manager users (rbenv, asdf) get
//! installs into their currently-selected ruby; system-ruby users get a
//! global install (sudo may be required out of band).
//!
//! Lock-file workflows (`bundle install` against a project `Gemfile.lock`)
//! are intentionally out of scope; that's a per-project concern handled by
//! the project's own bootstrap, not by `jarvy setup`.

use super::common::{
    PackageError, command_exists, run_package_command, validate_package_name,
    validate_package_version,
};
use super::config::{GemConfig, PackageSpec};

/// Handler for Ruby gem installation
pub struct GemHandler {
    config: GemConfig,
}

impl GemHandler {
    /// Create a new gem handler
    pub fn new(config: GemConfig) -> Self {
        Self { config }
    }

    /// Install all configured gems
    pub fn install(&self) -> Result<(), PackageError> {
        if !command_exists("gem") {
            return Err(PackageError::PackageManagerNotInstalled("gem".to_string()));
        }

        if self.config.packages.is_empty() {
            println!("    No gem packages configured");
            return Ok(());
        }

        // `gem install` is global to the active ruby; cwd is irrelevant.
        // Resolve once so a deleted-cwd doesn't surface as a mid-loop
        // install failure (same rationale as nuget.rs).
        let current_dir = std::env::current_dir().map_err(PackageError::Io)?;

        for (name, spec) in &self.config.packages {
            if spec.is_optional() {
                continue;
            }
            if let Err(e) = self.install_gem(name, spec, &current_dir) {
                tracing::warn!(
                    event = "package.install_failed",
                    ecosystem = "gem",
                    package = %name,
                    error = %e,
                );
                eprintln!("    Warning: Failed to install {}: {}", name, e);
            }
        }

        Ok(())
    }

    fn install_gem(
        &self,
        name: &str,
        spec: &PackageSpec,
        working_dir: &std::path::Path,
    ) -> Result<(), PackageError> {
        validate_package_name(name, "[gem]")?;
        validate_package_version(spec.version(), "[gem]")?;

        println!("    Installing {}...", name);
        let telemetry_on = crate::observability::telemetry_gate::is_enabled();
        if telemetry_on {
            tracing::info!(
                event = "package.requested",
                ecosystem = "gem",
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
            ecosystem = "gem",
            name = %name,
            version = %spec.version(),
        )
        .entered();
        match run_package_command("gem", &args, working_dir) {
            Ok(()) => {
                if telemetry_on {
                    tracing::info!(
                        event = "package.installed",
                        ecosystem = "gem",
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
                        ecosystem = "gem",
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

/// Build the argv passed to `gem`. `--no-document` is set unconditionally
/// — provisioning runs don't need RDoc/RI for global tooling, and skipping
/// the build cuts install time from ~30s to ~3s for chatty gems like
/// `rubocop`. `-v <version>` only when not "latest".
pub(crate) fn build_install_args<'a>(name: &'a str, version: &'a str) -> Vec<&'a str> {
    let mut args: Vec<&str> = Vec::with_capacity(5);
    args.extend_from_slice(&["install", "--no-document", name]);
    if version != "latest" {
        args.push("-v");
        args.push(version);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn gem_handler_empty() {
        let config = GemConfig::default();
        let handler = GemHandler::new(config);
        assert!(handler.config.packages.is_empty());
    }

    #[test]
    fn gem_handler_holds_packages() {
        let mut packages = HashMap::new();
        packages.insert(
            "rubocop".to_string(),
            PackageSpec::Version("latest".to_string()),
        );
        packages.insert(
            "bundler".to_string(),
            PackageSpec::Version("2.5.0".to_string()),
        );
        let config = GemConfig { packages };
        let handler = GemHandler::new(config);
        assert_eq!(handler.config.packages.len(), 2);
    }

    /// Pin the argv contract — `--no-document` must stay (the speed
    /// difference matters), `install` must stay (not `update`, which
    /// errors on first install), and the `-v` form (not `--version`)
    /// matches what `gem` documents.
    #[test]
    fn build_install_args_table() {
        let cases = [
            (
                "rubocop",
                "latest",
                vec!["install", "--no-document", "rubocop"],
            ),
            (
                "bundler",
                "2.5.0",
                vec!["install", "--no-document", "bundler", "-v", "2.5.0"],
            ),
        ];
        for (name, version, expected) in cases {
            let actual = build_install_args(name, version);
            assert_eq!(actual, expected, "argv mismatch for {} = {}", name, version);
            assert_eq!(actual[0], "install");
            assert_eq!(actual[1], "--no-document");
        }
    }

    /// Flag-like gem names must be refused before they hit `gem`.
    #[test]
    fn gem_rejects_flag_like_names() {
        let mut packages = HashMap::new();
        packages.insert(
            "--source".to_string(),
            PackageSpec::Version("latest".to_string()),
        );
        let config = GemConfig { packages };
        let handler = GemHandler::new(config);
        let result = handler.install();
        match &result {
            Ok(()) | Err(PackageError::PackageManagerNotInstalled(_)) => {}
            Err(other) => panic!("unexpected outer error: {other:?}"),
        }
        let spec = PackageSpec::Version("latest".to_string());
        let err = handler
            .install_gem("--source", &spec, std::path::Path::new("."))
            .expect_err("flag-like name must be refused");
        assert!(matches!(err, PackageError::RefusedUnsafeSpec(_, _)));
    }
}
