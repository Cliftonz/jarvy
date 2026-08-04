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
    // Defense-in-depth: route.package is a compile-time constant, but it
    // still passes the same gauntlet as user-supplied package entries.
    if validate_package_name(route.package, "fallback_route").is_err() {
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
    match route.eco {
        FallbackEco::Go => {
            let spec_arg = match pin.as_deref() {
                Some(p) => format!("{}@v{p}", route.package),
                None => format!("{}@latest", route.package),
            };
            println!("  Installing {} via `go install {spec_arg}`", spec.name);
            run("go", &["install", &spec_arg])?;
        }
        FallbackEco::Npm => {
            let spec_arg = match pin.as_deref() {
                Some(p) => format!("{}@{p}", route.package),
                None => route.package.to_string(),
            };
            println!("  Installing {} via `npm install -g {spec_arg}`", spec.name);
            run("npm", &["install", "-g", &spec_arg])?;
        }
        FallbackEco::Cargo => {
            // `--locked` mirrors `install_via_cargo_install` — the
            // supply-chain contract for every cargo route in jarvy.
            println!(
                "  Installing {} via `cargo install --locked {}`",
                spec.name, route.package
            );
            match pin.as_deref() {
                Some(p) => run(
                    "cargo",
                    &["install", "--locked", route.package, "--version", p],
                )?,
                None => run("cargo", &["install", "--locked", route.package])?,
            };
        }
        FallbackEco::Uv => {
            let spec_arg = match pin.as_deref() {
                Some(p) => format!("{}=={p}", route.package),
                None => route.package.to_string(),
            };
            println!(
                "  Installing {} via `uv tool install {spec_arg}`",
                spec.name
            );
            run("uv", &["tool", "install", &spec_arg])?;
        }
    }
    Ok(())
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
    let route_name = match route.eco {
        FallbackEco::Go => "go",
        FallbackEco::Npm => "npm",
        FallbackEco::Cargo => "cargo",
        FallbackEco::Uv => "uv",
    };
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
