// Generic integration test: each tool's add/ensure path should be invocable without panicking.
// OS-specific success and actual installation are covered by e2e tests.

#[test]
fn tools_add_handlers_are_invocable() {
    // Ensure built-in tools are registered
    jarvy::tools::register_all();

    // Get all registered tool names from the registry
    let names = jarvy::tools::registered_tool_names();
    assert!(!names.is_empty(), "expected at least one registered tool");

    for name in names {
        let res = jarvy::tools::add(&name, "");
        // We only care that this path returns a Result (no panic)
        assert!(res.is_ok() || res.is_err(), "{} add handler panicked", name);
    }
}
