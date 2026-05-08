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
    // A ToolSpec with no macos/linux/windows package AND no custom_install
    // would silently fail every install attempt with "no platform support".
    // Catch the configuration error at the test layer instead of at runtime.
    let mut offenders: Vec<&'static str> = Vec::new();
    for entry in iter_tools() {
        let spec: &ToolSpec = entry.spec;
        let any_platform = spec.macos.is_some()
            || spec.linux.is_some()
            || spec.windows.is_some()
            || spec.bsd.is_some()
            || spec.custom_install.is_some();
        if !any_platform {
            offenders.push(spec.name);
        }
    }
    assert!(
        offenders.is_empty(),
        "tool specs with no platform install and no custom_install: {offenders:?}"
    );
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
