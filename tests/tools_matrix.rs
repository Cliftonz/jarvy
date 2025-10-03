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
