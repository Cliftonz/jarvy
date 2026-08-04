//! Ecosystem fallback installer runtime (PRD-060 Phase 1).
//!
//! When a tool has no platform installer for the current OS
//! (`InstallError::Unsupported`), `ToolSpec::install()` hands off here.
//! Routes declared in the `fallback:` slot are tried in order; each
//! route needs its ecosystem toolchain (`go` / `npm` / `cargo` / `uv`)
//! on PATH — if missing, jarvy bootstraps it through its OWN registry
//! (go / node / rust / uv all have first-party platform routes), then
//! re-checks PATH. A toolchain that still isn't available marks the
//! route blocked and the next route is tried; an actual install-command
//! failure surfaces immediately (no silent cascade). All routes
//! exhausted → `InstallError::Unsupported` so setup's existing
//! `tool.unsupported` discrimination fires.
//!
//! HARD REQUIREMENT: ecosystem package managers only — no `curl | sh`
//! routes, ever. Package identifiers are compile-time first-party
//! constants; `validate_package_name` / `validate_package_version`
//! run anyway as defense-in-depth before anything reaches an argv.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::observability::telemetry_gate::is_enabled;
use crate::packages::common::{validate_package_name, validate_package_version};
use crate::tools::common::{InstallError, has, run};
use crate::tools::spec::{FallbackEco, FallbackRoute, ToolSpec};

/// Per-tool record of the route that succeeded: (route label, toolchain
/// bootstrapped). `telemetry::tool_installed` queries this so call
/// sites keep their signatures.
static ROUTES: OnceLock<Mutex<HashMap<String, (&'static str, bool)>>> = OnceLock::new();
/// Per-tool record of why fallback could not run, for `tool.unsupported`
/// enrichment ("toolchain_uninstallable" when every declared route was
/// blocked on its toolchain).
static BLOCKED: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();

fn routes() -> &'static Mutex<HashMap<String, (&'static str, bool)>> {
    ROUTES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn blocked() -> &'static Mutex<HashMap<String, &'static str>> {
    BLOCKED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Route that installed `tool` this process, if fallback did.
pub fn route_for(tool: &str) -> Option<(&'static str, bool)> {
    routes().lock().ok()?.get(tool).copied()
}

/// Why fallback was exhausted for `tool` this process, if it was.
pub fn blocked_reason_for(tool: &str) -> Option<&'static str> {
    blocked().lock().ok()?.get(tool).copied()
}

/// Enrichment for `tool.unsupported`: (fallback_declared,
/// fallback_blocked). Blocked is `"toolchain_uninstallable"` when every
/// declared route died on its toolchain this process, `"none_declared"`
/// when the tool has no routes at all.
pub fn unsupported_info(tool: &str) -> (bool, &'static str) {
    // Mirror registry::get_tool's dash ↔ underscore aliasing.
    let key = tool.to_ascii_lowercase();
    let alias_dash = key.replace('_', "-");
    let alias_underscore = key.replace('-', "_");
    let declared = crate::tools::spec::iter_tools()
        .map(|entry| entry.spec)
        .any(|s| {
            let sname = s.name.to_ascii_lowercase();
            (sname == key || sname == alias_dash || sname == alias_underscore)
                && !s.fallback.is_empty()
        });
    if !declared {
        return (false, "none_declared");
    }
    (
        true,
        blocked_reason_for(tool).unwrap_or("toolchain_uninstallable"),
    )
}

/// Try the declared fallback routes in order. Called by
/// `ToolSpec::install()` only when the primary path returned
/// `Unsupported` and at least one route is declared.
pub fn install_via_fallback(spec: &ToolSpec, min_hint: &str) -> Result<(), InstallError> {
    let mut any_blocked = false;
    for route in spec.fallback {
        match ensure_toolchain(spec.name, route.eco) {
            Ok(bootstrapped) => {
                run_route(spec, route, min_hint)?;
                if let Ok(mut map) = routes().lock() {
                    map.insert(
                        spec.name.to_string(),
                        (route.eco.route_label(), bootstrapped),
                    );
                }
                write_receipt(spec, route, min_hint, bootstrapped);
                return Ok(());
            }
            Err(reason) => {
                any_blocked = true;
                println!(
                    "  {} route unavailable for {}: {reason} — trying next route",
                    route.eco.command(),
                    spec.name
                );
                if is_enabled() {
                    tracing::warn!(
                        event = "tool.fallback_failed",
                        tool = spec.name,
                        eco = route.eco.command(),
                        error_kind = "toolchain_unavailable",
                    );
                }
            }
        }
    }
    if any_blocked && let Ok(mut map) = blocked().lock() {
        map.insert(spec.name.to_string(), "toolchain_uninstallable");
    }
    Err(InstallError::Unsupported)
}

/// Make sure the route's toolchain command is on PATH, bootstrapping it
/// through jarvy's own registry when missing. Returns whether a
/// bootstrap happened. `Err` = route unusable (bounded reason).
fn ensure_toolchain(tool: &str, eco: FallbackEco) -> Result<bool, &'static str> {
    if has(eco.command()) {
        return Ok(false);
    }
    println!(
        "  {} needs {} — installing {} via jarvy first",
        tool,
        eco.command(),
        eco.toolchain_tool()
    );
    if crate::tools::registry::add(eco.toolchain_tool(), "latest").is_err() {
        return Err("toolchain install failed");
    }
    // Freshly installed toolchains may need a new shell for PATH
    // (winget PATH edits, nvm sourcing) — evict the stale `has()` cache
    // entry and re-check rather than assume.
    crate::tools::common::forget_has(eco.command());
    if has(eco.command()) {
        Ok(true)
    } else {
        Err("toolchain installed but not on PATH in this shell — open a new terminal and re-run")
    }
}

/// Run the ecosystem install command for one route.
fn run_route(spec: &ToolSpec, route: &FallbackRoute, min_hint: &str) -> Result<(), InstallError> {
    run_eco(
        spec.name,
        route.eco,
        route.package,
        min_hint,
        EcoOp::Install,
    )
}

/// Re-run a receipt's ecosystem route to upgrade (PRD-060 Phase 2).
/// The package here comes from the user-writable receipts file —
/// `run_eco` runs the same validation gauntlet as the install path.
/// Callers must verify the toolchain is on PATH first (no bootstrap
/// on upgrade — the toolchain installed the tool in the first place).
pub fn upgrade_via_route(
    tool: &str,
    eco: FallbackEco,
    package: &str,
    version_hint: &str,
) -> Result<(), InstallError> {
    run_eco(tool, eco, package, version_hint, EcoOp::Upgrade)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EcoOp {
    Install,
    Upgrade,
}

/// Shared install/upgrade runner. `package` is `&str` (not `'static`)
/// because the upgrade path feeds it from a receipt; the gauntlet
/// below is therefore load-bearing for both callers.
fn run_eco(
    tool: &str,
    eco: FallbackEco,
    package: &str,
    min_hint: &str,
    op: EcoOp,
) -> Result<(), InstallError> {
    // Defense-in-depth on install (package is a compile-time constant);
    // the whole defense on upgrade (package is receipt-sourced).
    if validate_package_name(package, "fallback_route").is_err() {
        return Err(InstallError::Parse("fallback package id failed validation"));
    }
    let pin = concrete_pin(min_hint);
    if let Some(p) = pin.as_deref()
        && validate_package_version(p, "fallback_route").is_err()
    {
        return Err(InstallError::Parse(
            "fallback package version failed validation",
        ));
    }
    let verb = match op {
        EcoOp::Install => "Installing",
        EcoOp::Upgrade => "Upgrading",
    };
    match eco {
        FallbackEco::Go => {
            // `go install pkg@version` is already idempotent-upgrade.
            let spec_arg = match pin.as_deref() {
                Some(p) => format!("{package}@v{p}"),
                None => format!("{package}@latest"),
            };
            println!("  {verb} {tool} via `go install {spec_arg}`");
            run("go", &["install", &spec_arg])?;
        }
        FallbackEco::Npm => {
            // `npm install -g` replaces an existing global in place.
            let spec_arg = match pin.as_deref() {
                Some(p) => format!("{package}@{p}"),
                None => package.to_string(),
            };
            println!("  {verb} {tool} via `npm install -g {spec_arg}`");
            run("npm", &["install", "-g", &spec_arg])?;
        }
        FallbackEco::Cargo => {
            // `--locked` mirrors `install_via_cargo_install` — the
            // supply-chain contract for every cargo route in jarvy.
            // Upgrade adds `--force`: cargo refuses to overwrite an
            // existing binary otherwise.
            let mut args = vec!["install", "--locked"];
            if op == EcoOp::Upgrade {
                args.push("--force");
            }
            args.push(package);
            if let Some(p) = pin.as_deref() {
                args.push("--version");
                args.push(p);
            }
            println!("  {verb} {tool} via `cargo {}`", args.join(" "));
            run("cargo", &args)?;
        }
        FallbackEco::Uv => {
            // Upgrade uses `--reinstall`: re-resolves (latest when
            // unpinned) and replaces the existing tool env.
            let spec_arg = match pin.as_deref() {
                Some(p) => format!("{package}=={p}"),
                None => package.to_string(),
            };
            let mut args = vec!["tool", "install"];
            if op == EcoOp::Upgrade {
                args.push("--reinstall");
            }
            args.push(&spec_arg);
            println!("  {verb} {tool} via `uv {}`", args.join(" "));
            run("uv", &args)?;
        }
    }
    Ok(())
}

/// Preview the fallback route `tool` would take on THIS platform, for
/// `jarvy setup --dry-run` / `jarvy diff`. `Some((eco_label, command))`
/// — e.g. `("go", "go install github.com/x/y@latest")` — only when the
/// tool has no platform installer here, no custom installer, and
/// declares at least one route (mirrors the runtime's handoff
/// condition; first declared route shown, matching try order).
pub fn preview_route(tool: &str, min_hint: &str) -> Option<(&'static str, String)> {
    let spec = crate::tools::spec::get_tool_spec(tool)?;
    if spec.custom_install.is_some()
        || crate::tools::spec::get_tool_install_info(tool, min_hint).is_some()
    {
        return None;
    }
    let route = spec.fallback.first()?;
    Some((
        route.eco.receipt_route(),
        preview_command(route.eco, route.package, min_hint),
    ))
}

/// Display string for a fallback install — mirrors `run_eco`'s install argv.
fn preview_command(eco: FallbackEco, package: &str, min_hint: &str) -> String {
    let pin = concrete_pin(min_hint);
    match eco {
        FallbackEco::Go => match pin.as_deref() {
            Some(p) => format!("go install {package}@v{p}"),
            None => format!("go install {package}@latest"),
        },
        FallbackEco::Npm => match pin.as_deref() {
            Some(p) => format!("npm install -g {package}@{p}"),
            None => format!("npm install -g {package}"),
        },
        FallbackEco::Cargo => match pin.as_deref() {
            Some(p) => format!("cargo install --locked {package} --version {p}"),
            None => format!("cargo install --locked {package}"),
        },
        FallbackEco::Uv => match pin.as_deref() {
            Some(p) => format!("uv tool install {package}=={p}"),
            None => format!("uv tool install {package}"),
        },
    }
}

/// Map a config version hint to a concrete pin. Concrete = digits and
/// dots only (optional leading `v`), non-empty segments, ≤3 segments —
/// the charset check doubles as the injection guard since the value
/// lands in an ecosystem-installer argv. Everything else ("latest",
/// ranges, wildcards, empty) means "latest".
pub fn concrete_pin(version: &str) -> Option<String> {
    let v = version.trim().trim_start_matches('v');
    let concrete = !v.is_empty()
        && v.chars().all(|c| c.is_ascii_digit() || c == '.')
        && v.split('.').all(|p| !p.is_empty())
        && v.split('.').count() <= 3;
    concrete.then(|| v.to_string())
}

/// Best-effort receipt write — the install already succeeded, so a
/// failed receipt never fails the route.
fn write_receipt(spec: &ToolSpec, route: &FallbackRoute, min_hint: &str, bootstrapped: bool) {
    let route_name = route.eco.receipt_route();
    if let Err(e) = crate::tools::receipts::record(
        spec.name,
        spec.command,
        route_name,
        route.package,
        min_hint,
        bootstrapped,
    ) && is_enabled()
    {
        tracing::warn!(
            event = "tool.receipt_write_failed",
            tool = spec.name,
            error_kind = e.kind(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concrete_pins_are_recognized() {
        assert_eq!(concrete_pin("1.7.3").as_deref(), Some("1.7.3"));
        assert_eq!(concrete_pin("v1.7").as_deref(), Some("1.7"));
        assert_eq!(concrete_pin(" 2 ").as_deref(), Some("2"));
    }

    #[test]
    fn non_concrete_hints_mean_latest() {
        for hint in [
            "latest", "", "*", ">=1.7", "~1.7", "1.x", "1..3", ".1", "1.2.3.4",
        ] {
            assert_eq!(concrete_pin(hint), None, "hint {hint:?}");
        }
    }

    #[test]
    fn preview_command_mirrors_install_argv() {
        assert_eq!(
            preview_command(FallbackEco::Go, "github.com/x/y", "latest"),
            "go install github.com/x/y@latest"
        );
        assert_eq!(
            preview_command(FallbackEco::Go, "github.com/x/y", "1.2.3"),
            "go install github.com/x/y@v1.2.3"
        );
        assert_eq!(
            preview_command(FallbackEco::Npm, "allure-commandline", "2.30.0"),
            "npm install -g allure-commandline@2.30.0"
        );
        assert_eq!(
            preview_command(FallbackEco::Cargo, "cargo-deny", "latest"),
            "cargo install --locked cargo-deny"
        );
        assert_eq!(
            preview_command(FallbackEco::Uv, "cfn-lint", "1.5"),
            "uv tool install cfn-lint==1.5"
        );
    }

    #[test]
    fn preview_route_gates_on_platform_and_custom_install() {
        // Unknown tool → None.
        assert!(preview_route("no_such_tool", "latest").is_none());
        // jq has a platform installer on every supported OS → None.
        assert!(preview_route("jq", "latest").is_none());
        // rust is custom_install → None even with no PM slot.
        assert!(preview_route("rust", "latest").is_none());
    }

    #[test]
    fn route_recording_roundtrips() {
        assert!(route_for("no_such_tool").is_none());
        routes()
            .lock()
            .expect("lock")
            .insert("fake_tool".to_string(), ("fallback_go", true));
        assert_eq!(route_for("fake_tool"), Some(("fallback_go", true)));
    }

    #[test]
    fn eco_metadata_is_consistent() {
        for eco in [
            FallbackEco::Go,
            FallbackEco::Npm,
            FallbackEco::Cargo,
            FallbackEco::Uv,
        ] {
            assert!(eco.route_label().starts_with("fallback_"));
            assert!(!eco.command().is_empty());
            assert!(!eco.toolchain_tool().is_empty());
        }
    }
}
