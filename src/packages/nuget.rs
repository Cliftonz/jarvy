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
    PackageError, command_exists, run_install_loop, run_package_command, validate_package_name,
    validate_package_version,
};
use super::config::{NugetConfig, OnNewerInstalledPolicy, PackageSpec};
use std::collections::HashMap;
use std::path::Path;

/// Handler for .NET global tool installation
pub struct NugetHandler {
    config: NugetConfig,
}

impl NugetHandler {
    /// Create a new nuget handler
    pub fn new(config: NugetConfig) -> Self {
        Self { config }
    }

    /// Install all configured global tools. Idempotent via `dotnet tool
    /// update -g` (rather than `install -g` which errors when the tool
    /// is already present).
    ///
    /// `Strict`-policy packages (the default) go through the unchanged
    /// shared install loop. `Skip`/`Warn` packages are pulled out first:
    /// `Skip` needs a `dotnet tool list -g` query to decide up front
    /// whether to attempt the update at all; `Warn` needs to inspect the
    /// real failure after attempting it, which the shared loop's
    /// argv-only closure contract can't express, so it's run directly
    /// here via the same `run_package_command` primitive.
    pub fn install(&self) -> Result<(), PackageError> {
        if self.config.packages.is_empty() {
            return run_install_loop(
                "nuget",
                "dotnet",
                "[nuget]",
                "No NuGet global tools configured",
                &self.config.packages,
                |name, spec| Ok(build_install_args(name, spec.version())),
            );
        }

        let (standard, warn_packages) = self.partition_packages();

        if !standard.is_empty() {
            run_install_loop(
                "nuget",
                "dotnet",
                "[nuget]",
                "No NuGet global tools configured",
                &standard,
                |name, spec| Ok(build_install_args(name, spec.version())),
            )?;
        } else if !command_exists("dotnet") {
            return Err(PackageError::PackageManagerNotInstalled("dotnet".into()));
        }

        if !warn_packages.is_empty() {
            let current_dir = std::env::current_dir().map_err(PackageError::Io)?;
            let mut failed_names: Vec<String> = Vec::new();
            for (name, spec) in &warn_packages {
                if let Err(e) = install_with_warn_policy(name, spec, &current_dir) {
                    tracing::warn!(
                        event = "package.install_failed",
                        ecosystem = "nuget",
                        package = %name,
                        error = %e,
                    );
                    eprintln!("    Warning: Failed to install {}: {}", name, e);
                    failed_names.push(name.clone());
                }
            }
            if !failed_names.is_empty() {
                return Err(PackageError::InstallFailed(format!(
                    "nuget global tool(s) failed under on_newer_installed = \"warn\": {}",
                    failed_names.join(", ")
                )));
            }
        }

        Ok(())
    }

    /// Splits configured packages into the set that goes through the
    /// shared install loop unchanged (`Strict`, plus `Skip` packages that
    /// don't actually need skipping) and the `Warn`-policy set handled
    /// separately. Optional packages are dropped here for `Skip`/`Warn`
    /// to mirror `run_install_loop`'s own optional-skip for `Strict`.
    fn partition_packages(&self) -> (HashMap<String, PackageSpec>, Vec<(String, PackageSpec)>) {
        let needs_query = self
            .config
            .packages
            .values()
            .any(|s| s.on_newer_installed() == OnNewerInstalledPolicy::Skip);
        let installed_list = if needs_query {
            let queried = query_installed_tools();
            if queried.is_none() {
                eprintln!(
                    "    Warning: could not query installed .NET global tools (dotnet tool list -g failed); the Skip policy will not apply this run"
                );
            }
            queried
        } else {
            None
        };

        let mut standard = HashMap::new();
        let mut warn_packages = Vec::new();

        for (name, spec) in &self.config.packages {
            if spec.is_optional() {
                continue;
            }
            match spec.on_newer_installed() {
                OnNewerInstalledPolicy::Strict => {
                    standard.insert(name.clone(), spec.clone());
                }
                OnNewerInstalledPolicy::Skip => {
                    let installed = installed_list
                        .as_deref()
                        .and_then(|out| parse_installed_version(out, name));
                    if should_skip(
                        OnNewerInstalledPolicy::Skip,
                        spec.version(),
                        installed.as_deref(),
                    ) {
                        println!(
                            "    Skipping {name}: pinned {} already satisfied by installed {}",
                            spec.version(),
                            installed.as_deref().unwrap_or("?"),
                        );
                    } else {
                        standard.insert(name.clone(), spec.clone());
                    }
                }
                OnNewerInstalledPolicy::Warn => {
                    warn_packages.push((name.clone(), spec.clone()));
                }
            }
        }

        (standard, warn_packages)
    }
}

/// Runs `dotnet tool list -g` and returns its stdout, or `None` on any
/// spawn/exit failure. Callers fall through to `Strict`-equivalent
/// behavior (never skip) when the query itself is unavailable.
fn query_installed_tools() -> Option<String> {
    let out = crate::tools::common::run("dotnet", &["tool", "list", "-g"]).ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parses `dotnet tool list -g` table output to find the installed
/// version of `tool_name`. Rows are whitespace-separated
/// `<PackageId> <Version> <Commands...>`; the header row (`Package Id
/// Version Commands`) and the dashed separator row never have a first
/// token matching a real tool name, so they fall out naturally without
/// special-casing. Matches case-insensitively since NuGet tool ids are
/// conventionally lowercase but that isn't guaranteed.
pub(crate) fn parse_installed_version(output: &str, tool_name: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut tokens = line.split_whitespace();
        let id = tokens.next()?;
        let version = tokens.next()?;
        id.eq_ignore_ascii_case(tool_name)
            .then(|| version.to_string())
    })
}

/// Decides whether a `Skip`-policy package's update should be skipped:
/// true only when the pin is a valid, concrete version and the installed
/// version is present and >= the pin. Non-`Skip` policies, an unparseable
/// pin (e.g. `"latest"`), or a missing/unparseable installed version all
/// fall through to `false` (attempt the update), matching `Strict`.
/// Non-semver strings fall back to exact equality, mirroring
/// `VersionPolicy`'s fallback in `src/drift/config.rs`.
pub(crate) fn should_skip(
    policy: OnNewerInstalledPolicy,
    pinned: &str,
    installed: Option<&str>,
) -> bool {
    if policy != OnNewerInstalledPolicy::Skip {
        return false;
    }
    let Some(installed) = installed else {
        return false;
    };
    match (
        semver::Version::parse(pinned),
        semver::Version::parse(installed),
    ) {
        (Ok(p), Ok(i)) => i >= p,
        _ => installed == pinned,
    }
}

/// True when a failed `dotnet tool update -g` message matches the
/// downgrade-refusal shape: "The requested version X is lower than
/// existing version Y." Requires both substrings (case-insensitive) so
/// other genuine `dotnet` failures aren't misclassified, mirroring
/// `winget_reports_already_installed`'s two-substring style in
/// `src/tools/common.rs`.
pub(crate) fn is_newer_installed_conflict(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("requested version") && lower.contains("is lower than existing version")
}

/// True when a `dotnet tool update -g` result is the specific
/// downgrade-refusal shape that `Warn` policy suppresses. Factored out
/// of `install_with_warn_policy`'s match arms so the routing decision
/// is unit-testable without spawning `dotnet`.
fn is_suppressible_warn_failure(result: &Result<(), PackageError>) -> bool {
    matches!(result, Err(PackageError::CommandFailed(msg)) if is_newer_installed_conflict(msg))
}

/// Attempts the pinned update for a `Warn`-policy package directly
/// (bypassing the shared install loop, which has no hook to inspect a
/// post-execution failure). A downgrade-refusal failure is logged as a
/// warning and treated as handled; any other failure propagates.
fn install_with_warn_policy(
    name: &str,
    spec: &PackageSpec,
    working_dir: &Path,
) -> Result<(), PackageError> {
    validate_package_name(name, "[nuget]")?;
    validate_package_version(spec.version(), "[nuget]")?;

    println!("    Installing {}...", name);
    let args_owned = build_install_args(name, spec.version());
    let args: Vec<&str> = args_owned.iter().map(String::as_str).collect();
    let result = run_package_command("dotnet", &args, working_dir);
    if is_suppressible_warn_failure(&result) {
        eprintln!(
            "    Warning: {name} pin ({}) is older than the installed global tool version; leaving the existing install in place",
            spec.version()
        );
        return Ok(());
    }
    result
}

/// Build the argv passed to `dotnet`. Pinned by a unit test below so
/// the `tool update -g` (idempotent) shape can't silently regress.
pub(crate) fn build_install_args(name: &str, version: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::with_capacity(6);
    args.push("tool".into());
    args.push("update".into());
    args.push("-g".into());
    args.push(name.into());
    if version != "latest" {
        args.push("--version".into());
        args.push(version.into());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::super::config::PackageSpec;
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
            let actual_refs: Vec<&str> = actual.iter().map(String::as_str).collect();
            assert_eq!(
                actual_refs, expected,
                "argv mismatch for {} = {}",
                name, version
            );
            assert_eq!(actual[0], "tool", "first arg must be `tool`");
            assert_eq!(actual[1], "update", "must use `update` for idempotency");
            assert_eq!(actual[2], "-g", "must be global install");
            assert_ne!(actual[1], "install", "`install` errors when present");
        }
    }

    /// Flag-like nuget tool names must be refused before they hit
    /// `dotnet`. Wiring assertion only; full coverage in `common::tests`.
    #[test]
    fn nuget_rejects_flag_like_tool_names() {
        use super::super::common::validate_package_name;
        let err = validate_package_name("--source", "[nuget]")
            .expect_err("flag-like name must be refused");
        assert!(
            matches!(err, PackageError::RefusedUnsafeSpec(_, _)),
            "expected RefusedUnsafeSpec, got {err:?}"
        );
    }

    const DOTNET_TOOL_LIST_OUTPUT: &str = "\
Package Id      Version      Commands
--------------------------------------
dotnet-ef       10.0.9       dotnet-ef
csharpier       0.30.0       csharpier
";

    #[test]
    fn parse_installed_version_finds_present_tool() {
        assert_eq!(
            parse_installed_version(DOTNET_TOOL_LIST_OUTPUT, "dotnet-ef"),
            Some("10.0.9".to_string())
        );
        assert_eq!(
            parse_installed_version(DOTNET_TOOL_LIST_OUTPUT, "csharpier"),
            Some("0.30.0".to_string())
        );
    }

    #[test]
    fn parse_installed_version_is_case_insensitive() {
        assert_eq!(
            parse_installed_version(DOTNET_TOOL_LIST_OUTPUT, "Dotnet-EF"),
            Some("10.0.9".to_string())
        );
    }

    #[test]
    fn parse_installed_version_absent_tool_returns_none() {
        assert_eq!(
            parse_installed_version(DOTNET_TOOL_LIST_OUTPUT, "dotnet-outdated-tool"),
            None
        );
    }

    #[test]
    fn parse_installed_version_handles_malformed_and_empty_output() {
        assert_eq!(parse_installed_version("", "dotnet-ef"), None);
        assert_eq!(
            parse_installed_version("garbage\n\n---\n", "dotnet-ef"),
            None
        );
        assert_eq!(
            parse_installed_version("Package Id Version Commands\n", "dotnet-ef"),
            None
        );
        // A line with only one token (no version column) must not panic
        // or be mistaken for a match.
        assert_eq!(parse_installed_version("dotnet-ef\n", "dotnet-ef"), None);
    }

    #[test]
    fn should_skip_true_when_installed_satisfies_pin() {
        assert!(should_skip(
            OnNewerInstalledPolicy::Skip,
            "8.0.2",
            Some("10.0.9")
        ));
        assert!(should_skip(
            OnNewerInstalledPolicy::Skip,
            "8.0.2",
            Some("8.0.2")
        ));
    }

    #[test]
    fn should_skip_false_when_installed_is_older_than_pin() {
        assert!(!should_skip(
            OnNewerInstalledPolicy::Skip,
            "10.0.9",
            Some("8.0.2")
        ));
    }

    #[test]
    fn should_skip_false_when_nothing_installed() {
        assert!(!should_skip(OnNewerInstalledPolicy::Skip, "8.0.2", None));
    }

    #[test]
    fn should_skip_false_for_non_skip_policies() {
        assert!(!should_skip(
            OnNewerInstalledPolicy::Strict,
            "8.0.2",
            Some("10.0.9")
        ));
        assert!(!should_skip(
            OnNewerInstalledPolicy::Warn,
            "8.0.2",
            Some("10.0.9")
        ));
    }

    #[test]
    fn should_skip_non_semver_falls_back_to_string_equality() {
        assert!(should_skip(
            OnNewerInstalledPolicy::Skip,
            "abc123",
            Some("abc123")
        ));
        assert!(!should_skip(
            OnNewerInstalledPolicy::Skip,
            "abc123",
            Some("abc124")
        ));
    }

    #[test]
    fn is_newer_installed_conflict_matches_dotnet_downgrade_message() {
        let msg = "'dotnet tool update -g dotnet-ef --version 8.0.2' exited with status 1\n\
                    --- last output ---\n\
                    The requested version 8.0.2 is lower than existing version 10.0.9.";
        assert!(is_newer_installed_conflict(msg));
    }

    #[test]
    fn is_newer_installed_conflict_requires_both_substrings() {
        assert!(!is_newer_installed_conflict(
            "No package found matching input criteria."
        ));
        assert!(!is_newer_installed_conflict("requested version 8.0.2"));
        assert!(!is_newer_installed_conflict(
            "is lower than existing version 10.0.9"
        ));
    }

    // `should_skip(Skip, pin, None)`, the case where the installed-tools
    // query is unavailable for a `Skip`-policy package, is already
    // covered above by `should_skip_false_when_nothing_installed`, which
    // asserts exactly `!should_skip(OnNewerInstalledPolicy::Skip, "8.0.2",
    // None)`. Not duplicated here.

    /// Table test for `install_with_warn_policy`'s routing: a
    /// conflict-shaped `CommandFailed` must be suppressed (treated as
    /// `Ok`), while a non-conflict `CommandFailed`, a different
    /// `PackageError` variant, and a plain `Ok` must all fall through
    /// unsuppressed.
    #[test]
    fn warn_policy_routes_conflict_vs_real_error() {
        let conflict = Err(PackageError::CommandFailed(
            "'dotnet tool update -g x --version 1.0.0' exited with status 1\n\
             The requested version 1.0.0 is lower than existing version 2.0.0."
                .to_string(),
        ));
        assert!(
            is_suppressible_warn_failure(&conflict),
            "downgrade-conflict message must be suppressed"
        );

        let unrelated_command_failure = Err(PackageError::CommandFailed(
            "No package found matching input criteria.".to_string(),
        ));
        assert!(
            !is_suppressible_warn_failure(&unrelated_command_failure),
            "non-conflict CommandFailed must propagate"
        );

        let other_variant = Err(PackageError::PackageManagerNotInstalled("dotnet".into()));
        assert!(
            !is_suppressible_warn_failure(&other_variant),
            "non-CommandFailed variants must propagate"
        );

        assert!(
            !is_suppressible_warn_failure(&Ok(())),
            "a success must not be classified as suppressible"
        );
    }

    // `NugetHandler::install()`'s `!command_exists("dotnet")` early-return
    // branch (`PackageManagerNotInstalled`) is not covered here:
    // `command_exists` (via `tools::common::has`) does a real PATH lookup
    // with a process-global cache and has no test-mode/mock hook in this
    // codebase, so a test would either be fragile (depends on whether
    // `dotnet` happens to be on the test machine's PATH) or require
    // injecting a fake, which is out of this fix-up's manifest
    // (`src/packages/config.rs`, `src/packages/nuget.rs`).
}
