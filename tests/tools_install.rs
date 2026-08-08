//! Registry shape regressions.
//!
//! Replaces the previous tautological `is_ok() || is_err()` test (which
//! provably never failed). These assertions catch:
//!  - duplicate registered command names
//!  - tool specs that have neither a platform install nor a custom_install
//!  - registry shrinking unexpectedly (a refactor that drops `register_all`
//!    body would leave the registry empty and we'd ship a broken jarvy)

use jarvy::tools::spec::{ToolSpec, iter_tools};
use std::collections::HashSet;

#[test]
fn registry_has_at_least_the_expected_floor() {
    jarvy::tools::register_all();
    // We want a number that grows with the codebase but won't false-positive
    // on a single legitimate removal. ~50 is well below the actual count
    // (>150 as of v0.0.5) but high enough to catch register_all() being
    // gutted to a stub.
    let count = jarvy::tools::registered_tool_names().len();
    assert!(
        count >= 50,
        "registry has {count} tools — expected at least 50; \
         did register_all() lose its body?"
    );
}

#[test]
fn no_duplicate_registered_tool_names() {
    jarvy::tools::register_all();
    let names = jarvy::tools::registered_tool_names();
    let mut seen: HashSet<String> = HashSet::new();
    let mut dups: Vec<String> = Vec::new();
    for n in &names {
        let key = n.to_lowercase();
        if !seen.insert(key.clone()) {
            dups.push(key);
        }
    }
    assert!(dups.is_empty(), "duplicate tool names: {dups:?}");
}

#[test]
fn every_spec_has_either_a_platform_resolver_or_custom_install() {
    // A ToolSpec with no macos/linux/windows package, no custom_install,
    // AND no fallback route (PRD-060) would silently fail every install
    // attempt with "no platform support". Ecosystem-only tools (e.g.
    // cargo-tarpaulin, pm2) legitimately declare only `fallback`.
    // Catch the configuration error at the test layer instead of at runtime.
    let mut offenders: Vec<&'static str> = Vec::new();
    for entry in iter_tools() {
        let spec: &ToolSpec = entry.spec;
        let any_install_route = spec.macos.is_some()
            || spec.linux.is_some()
            || spec.windows.is_some()
            || spec.bsd.is_some()
            || spec.custom_install.is_some()
            || !spec.fallback.is_empty();
        if !any_install_route {
            offenders.push(spec.name);
        }
    }
    assert!(
        offenders.is_empty(),
        "tool specs with no platform install, no custom_install, and no fallback route: {offenders:?}"
    );
}

#[test]
fn every_declared_fallback_package_passes_the_install_gauntlet() {
    // EP-4: fallback packages are compile-time constants, but nothing
    // stopped a future contributor from typing a leading `-`, a scheme
    // prefix, or a control byte into a `define_tool!` invocation. The
    // resulting spec would slip past `iter_tools()` and only blow up
    // at install time in a user's terminal. Run every declared route's
    // package through the same gauntlet `[cargo]`/`[npm]`/… entries
    // face at load time so the boundary is enforced at test time.
    let mut offenders: Vec<String> = Vec::new();
    for entry in iter_tools() {
        let spec: &ToolSpec = entry.spec;
        for route in spec.fallback {
            let purpose: &'static str = match route.eco {
                jarvy::tools::spec::FallbackEco::Go => "[fallback-go]",
                jarvy::tools::spec::FallbackEco::Npm => "[fallback-npm]",
                jarvy::tools::spec::FallbackEco::Cargo => "[fallback-cargo]",
                jarvy::tools::spec::FallbackEco::Uv => "[fallback-uv]",
            };
            if let Err(e) = jarvy::packages::common::validate_package_name(route.package, purpose) {
                offenders.push(format!("{} ({}): {e}", spec.name, route.package));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "declared fallback packages failed the install gauntlet:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn grafana_ecosystem_tools_are_registered() {
    // Guard: a future refactor that drops `pub mod <name>;` from
    // src/tools/mod.rs, or a cfg gate around `inventory::submit!`, would
    // silently unregister the Grafana ecosystem sweep (commit fdf4986).
    // Per-tool shape tests can't catch that — they only prove the static
    // value. Fail loudly at the boundary instead.
    jarvy::tools::register_all();
    let names: HashSet<String> = jarvy::tools::registered_tool_names().into_iter().collect();
    for expected in ["alloy", "logcli", "mimirtool", "promtail", "tanka", "xk6"] {
        assert!(
            names.contains(expected),
            "{expected} not registered — check `pub mod {expected};` in \
             src/tools/mod.rs and the define_tool! invocation"
        );
    }
}

#[test]
fn add_handler_returns_a_result_without_panicking() {
    // Sanity: install attempts skip via JARVY_FAST_TEST and surface a Result.
    // This is intentionally narrow — we're only verifying handler dispatch
    // doesn't panic on any registered name.
    // SAFETY: test-only env mutation.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("JARVY_FAST_TEST", "1");
    }
    jarvy::tools::register_all();
    let names = jarvy::tools::registered_tool_names();
    assert!(!names.is_empty());
    for name in names {
        // Just call it; if it returns Ok or Err we're fine. The previous
        // assertion `is_ok() || is_err()` is provably trivially true and
        // was removed.
        let _ = jarvy::tools::add(&name, "");
    }
}
