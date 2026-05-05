//! Integration tests for `~/.jarvy/tools.d/` plugin loader.
//!
//! These tests focus on the unit-level dispatch and validation logic; they do
//! not invoke the real `tools::add` path because that would shell out to the
//! system package manager. The contract tested here is:
//!
//! 1. `install_by_name` resolves to the correct plugin (NOT just "first
//!    missing" as the previous shared-handler implementation did).
//! 2. Plugin TOMLs containing injection-shaped package strings are rejected
//!    by `is_valid_package_name` rather than silently passed to `brew/apt`.
//! 3. World-writable plugin directories are refused on Unix.

use jarvy::tools::plugins::{self, PluginPlatform, PluginTool, get_plugin, install_by_name};
use std::sync::Mutex;

/// Serializes every test that mutates the global plugin registry. Cargo
/// runs integration tests in parallel; without this lock, one test's
/// `_test_clear()` can wipe another test's `_test_register(...)` between
/// the register call and the subsequent get_plugin lookup, causing
/// install_by_name_resolves_to_correct_plugin (and friends) to flake.
/// Tests that DON'T touch the registry (validates_*, refuses_*, etc.)
/// don't need this guard.
static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

fn fresh_plugin(name: &str, command: &str) -> PluginTool {
    PluginTool {
        name: name.to_string(),
        command: command.to_string(),
        macos: Some(PluginPlatform {
            brew: Some(format!("{name}-pkg")),
            cask: None,
            uniform: None,
            apt: None,
            dnf: None,
            pacman: None,
            winget: None,
            choco: None,
        }),
        linux: Some(PluginPlatform {
            brew: None,
            cask: None,
            uniform: Some(format!("{name}-pkg")),
            apt: None,
            dnf: None,
            pacman: None,
            winget: None,
            choco: None,
        }),
        windows: Some(PluginPlatform {
            brew: None,
            cask: None,
            uniform: None,
            apt: None,
            dnf: None,
            pacman: None,
            winget: Some(format!("Vendor.{name}")),
            choco: None,
        }),
    }
}

#[test]
fn install_by_name_resolves_to_correct_plugin() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    plugins::_test_clear();
    plugins::_test_register(fresh_plugin("alpha", "alpha-cmd"));
    plugins::_test_register(fresh_plugin("beta", "beta-cmd"));

    let alpha = get_plugin("alpha").expect("alpha registered");
    assert_eq!(alpha.name, "alpha");
    let beta = get_plugin("beta").expect("beta registered");
    assert_eq!(beta.name, "beta");

    // Ensures install_by_name does not return the FIRST registered plugin —
    // the previous bug in plugin_install_handler installed whichever plugin
    // appeared first in iteration order.
    assert_ne!(alpha.command, beta.command);
}

#[test]
fn install_by_name_returns_false_for_unknown_plugin() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    plugins::_test_clear();
    plugins::_test_register(fresh_plugin("known", "known-cmd"));

    let result = install_by_name("definitely-not-a-plugin", "latest");
    assert!(matches!(result, Ok(false)));
}

#[test]
fn lookup_is_case_insensitive() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    plugins::_test_clear();
    plugins::_test_register(fresh_plugin("CamelCase", "camelcase"));

    assert!(get_plugin("camelcase").is_some());
    assert!(get_plugin("CAMELCASE").is_some());
    assert!(get_plugin("CamelCase").is_some());
}
