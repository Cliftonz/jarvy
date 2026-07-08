// Ensures a test target named `tools_matrix` exists for CI and performs a
// minimal sanity check over the built-in tools registry.
// This test is intentionally side‑effect free and deterministic.

#[test]
fn tools_matrix_is_registered_and_sorted() {
    // Register all built-in tools
    jarvy::tools::register_all();

    // Fetch the registered tool names (registry guarantees sorted order)
    let names = jarvy::tools::registered_tool_names();

    // There should be at least one tool registered
    assert!(
        !names.is_empty(),
        "expected at least one registered tool in the matrix"
    );

    // Names should already be sorted (documented contract of registered_tool_names)
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(
        names, sorted,
        "registered tool names should be sorted deterministically"
    );
}

/// The 10 tools added in the wizard/polyglot commit-pair must survive
/// registration into `tools::register_all()`. A future refactor that
/// splits `mod.rs` or gates a tool behind a `#[cfg]` flag would
/// silently drop it — this test names each one so the failure points
/// at the specific missing tool, not a bulk "tools_matrix broken"
/// message.
///
/// Complements the per-tool `_registration_shape` tests that were
/// deleted as macro-tautologies: those re-asserted the `define_tool!`
/// field assignments (already type-checked at compile time), whereas
/// this test asserts the registration side-effect actually landed.
#[test]
fn new_polyglot_tools_are_registered() {
    jarvy::tools::register_all();
    let names = jarvy::tools::registered_tool_names();
    for tool in [
        "bacon",
        "bazelisk",
        "cargo_nextest",
        "cmake",
        "composer",
        "infisical",
        "pnpm",
        "release_plz",
        "skaffold",
        "yarn",
    ] {
        assert!(
            names.iter().any(|n| n == tool),
            "tool `{tool}` must be in the registered set — did `mod.rs` \
             drop the `pub mod {tool};` line? got: {names:?}"
        );
    }
}

/// Same registration-side-effect guard for the new-tools batch
/// (tasks/new_tools.json queue drain + backlog picks). Each of these
/// must survive `register_all()`; a dropped `pub mod` line in
/// `src/tools/mod.rs` fails here by name.
#[test]
fn new_tools_batch_is_registered() {
    jarvy::tools::register_all();
    let names = jarvy::tools::registered_tool_names();
    for tool in [
        "allure",
        "aws_sam_cli",
        "cfn_lint",
        "cypress",
        "goaccess",
        "linkerd",
        "locust",
        "playwright",
        "putty",
        "task",
    ] {
        assert!(
            names.iter().any(|n| n == tool),
            "tool `{tool}` must be in the registered set — did `mod.rs` \
             drop the `pub mod {tool};` line? got: {names:?}"
        );
    }
}

/// The dash/underscore alias resolution in `tools::registry::get_tool()`
/// must find every new tool whose canonical form uses an underscore
/// (Rust identifiers can't contain dashes). Users typing the natural
/// `cargo-nextest = "latest"` in `jarvy.toml` must resolve to the
/// underscore-keyed `cargo_nextest` handler.
#[test]
fn dash_form_tool_names_resolve_via_aliasing() {
    jarvy::tools::register_all();
    for (dash, expected_underscore) in [
        ("cargo-nextest", "cargo_nextest"),
        ("release-plz", "release_plz"),
        ("aws-sam-cli", "aws_sam_cli"),
        ("cfn-lint", "cfn_lint"),
    ] {
        assert!(
            jarvy::tools::get_tool(dash).is_some(),
            "get_tool(`{dash}`) should resolve via dash→underscore \
             fallback to `{expected_underscore}` — the alias path in \
             registry::get_tool() covers this. If this test fails, the \
             alias code was removed or the tool was renamed."
        );
    }
}

/// QA F7: negative test for the alias path. A dash-form name that
/// doesn't correspond to any registered tool MUST return `None` —
/// the fallback must not accidentally match unrelated tools by
/// substring, prefix, or any other loose comparison.
#[test]
fn dash_form_alias_does_not_leak_to_bogus_names() {
    jarvy::tools::register_all();
    for bogus in [
        "bogus-tool",
        "bogus_tool",
        "not-a-real-tool",
        "release-plz-x",
    ] {
        assert!(
            jarvy::tools::get_tool(bogus).is_none(),
            "get_tool(`{bogus}`) MUST return None — the dash/underscore \
             alias fallback must be strict, not a prefix or substring \
             match. If this fails, the alias code may be over-matching."
        );
    }
}

/// QA F7 positive-case coverage for the 8 dash-free new tools. Their
/// canonical registration form contains no dash or underscore
/// difference from the TOML key, so they resolve via the exact-name
/// path — no alias fallback needed. If a future rename introduces an
/// underscore into any of these, the naive `get_tool(dash_form)`
/// would silently miss without this test.
#[test]
fn canonical_dash_free_tools_resolve_directly() {
    jarvy::tools::register_all();
    for name in [
        "pnpm",
        "yarn",
        "bun",
        "bazelisk",
        "composer",
        "infisical",
        "skaffold",
        "cmake",
    ] {
        assert!(
            jarvy::tools::get_tool(name).is_some(),
            "canonical `{name}` must resolve via exact-name path — if \
             this fails, either the tool was renamed or the exact-name \
             path in registry::get_tool() broke."
        );
    }
}
