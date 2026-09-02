//! graphify - AST-based knowledge graph generator for codebases
//!
//! Wraps the `graphify-dotnet` .NET global tool
//! (<https://github.com/elbruno/graphify-dotnet>,
//! <https://www.nuget.org/packages/graphify-dotnet>), which builds an
//! AST-based knowledge graph of a codebase, optionally AI-enriched.
//!
//! Verified via WebSearch/WebFetch Sept 2026: package id `graphify-dotnet`,
//! latest 0.7.0, MIT licensed, targets .NET 10. The installed CLI binary is
//! invoked as `graphify`.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, run};

// graphify-dotnet ships only as a NuGet global tool as of Sept 2026 — no
// first-party brew/winget/apt package exists, so every platform routes
// through custom_install.
define_tool!(GRAPHIFY, {
    command: "graphify",
    custom_install: install_graphify,
});

fn install_graphify(min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    // graphify-dotnet targets the .NET 10 SDK; jarvy's own `dotnet` tool is
    // pinned to .NET 8 and won't satisfy this. This is a hard prerequisite
    // the user must install themselves — jarvy does not auto-install or
    // upgrade the SDK on their behalf.
    if !has_dotnet_10_or_newer() {
        return Err(InstallError::Prereq(
            "graphify-dotnet requires the .NET 10 SDK (checked via `dotnet \
             --list-sdks`) — none found. jarvy's own `dotnet` tool is pinned \
             to .NET 8 and won't satisfy this. Install .NET 10 manually from \
             https://dotnet.microsoft.com/download/dotnet/10.0 and re-run."
                .into(),
        ));
    }

    let version = normalize_version(min_hint);
    crate::packages::common::validate_package_version(version, "[provisioner] graphify")
        .map_err(|e| InstallError::Prereq(e.to_string().into()))?;

    let args = dotnet_tool_update_args(version);
    run("dotnet", &args)?;
    Ok(())
}

/// Matches the "no pin requested" idiom `dotnet_tool_update_args` expects.
fn normalize_version(hint: &str) -> &str {
    if hint.is_empty() || hint == "*" {
        "latest"
    } else {
        hint
    }
}

/// `update` (not `install`) keeps this idempotent on a box where the tool
/// is already present. Local to this file — deliberately not shared with
/// `packages::nuget::build_install_args`, which lives across the
/// `tools`/`packages` boundary this tool doesn't reach into.
fn dotnet_tool_update_args(version: &str) -> Vec<&str> {
    let mut args = vec!["tool", "update", "-g", "graphify-dotnet"];
    if version != "latest" {
        args.push("--version");
        args.push(version);
    }
    args
}

/// Probes installed SDKs via `dotnet --list-sdks` rather than `dotnet
/// --version`/`cmd_satisfies`, which resolves through any `global.json` in
/// jarvy's cwd and would falsely refuse graphify when a newer SDK is
/// installed but an older one is pinned for the current directory.
/// Read-only probe, so — like `has()`/`cmd_version_output` in
/// `tools::common` — it runs unconditionally, without the `run()`
/// mutating-install test gate.
fn has_dotnet_10_or_newer() -> bool {
    let Ok(out) = std::process::Command::new("dotnet")
        .arg("--list-sdks")
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    any_major_at_least(&String::from_utf8_lossy(&out.stdout), 10)
}

fn any_major_at_least(stdout: &str, min_major: u32) -> bool {
    stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|v| v.split('.').next())
        .filter_map(|major| major.parse::<u32>().ok())
        .any(|major| major >= min_major)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphify_registration_shape() {
        assert_eq!(GRAPHIFY.command, "graphify");
        assert!(GRAPHIFY.macos.is_none());
        assert!(GRAPHIFY.linux.is_none());
        assert!(GRAPHIFY.windows.is_none());
        assert!(GRAPHIFY.bsd.is_none());
        assert!(GRAPHIFY.custom_install.is_some());
    }

    #[test]
    fn normalize_version_empty_is_latest() {
        assert_eq!(normalize_version(""), "latest");
    }

    #[test]
    fn normalize_version_wildcard_is_latest() {
        assert_eq!(normalize_version("*"), "latest");
    }

    #[test]
    fn normalize_version_pinned_is_unchanged() {
        assert_eq!(normalize_version("0.7.0"), "0.7.0");
    }

    #[test]
    fn normalize_version_latest_is_unchanged() {
        assert_eq!(normalize_version("latest"), "latest");
    }

    #[test]
    fn dotnet_tool_update_args_latest_omits_version_flag() {
        assert_eq!(
            dotnet_tool_update_args("latest"),
            vec!["tool", "update", "-g", "graphify-dotnet"]
        );
    }

    #[test]
    fn dotnet_tool_update_args_pinned_appends_version_flag() {
        assert_eq!(
            dotnet_tool_update_args("0.7.0"),
            vec![
                "tool",
                "update",
                "-g",
                "graphify-dotnet",
                "--version",
                "0.7.0"
            ]
        );
    }

    #[test]
    fn any_major_at_least_empty_output_is_false() {
        assert!(!any_major_at_least("", 10));
    }

    #[test]
    fn any_major_at_least_single_old_sdk_is_false() {
        assert!(!any_major_at_least(
            "8.0.401 [C:\\Program Files\\dotnet\\sdk]",
            10
        ));
    }

    #[test]
    fn any_major_at_least_single_new_sdk_is_true() {
        assert!(any_major_at_least(
            "10.0.100 [C:\\Program Files\\dotnet\\sdk]",
            10
        ));
    }

    #[test]
    fn any_major_at_least_mixed_old_and_new_is_true() {
        let stdout = "8.0.401 [C:\\Program Files\\dotnet\\sdk]\n\
                       10.0.100 [C:\\Program Files\\dotnet\\sdk]\n";
        assert!(any_major_at_least(stdout, 10));
    }
}
