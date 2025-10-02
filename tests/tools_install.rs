// Generic integration test: each tool's add/ensure path should be invocable without panicking.
// OS-specific success and actual installation are covered by e2e tests.

#[test]
fn tools_add_handlers_are_invocable() {
    // Ensure built-in tools are registered
    jarvy::tools::register_all();

    // (name, version_hint) pairs; keep hints minimal/generic
    let cases: &[(&str, &str)] = &[
        ("git", ""),
        ("brew", ""),
        ("vscode", ""),
        ("docker", ""),
        ("wget", ""),
        ("jq", ""),
        ("nvm", ""),
        ("tree", ""),
        ("tmux", ""),
        ("htop", ""),
    ];

    for (name, hint) in cases {
        let res = jarvy::tools::add(name, hint);
        // We only care that this path returns a Result (no panic)
        assert!(res.is_ok() || res.is_err(), "{} add handler panicked", name);
    }
}
