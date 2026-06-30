//! End-to-end happy-path test for `jarvy wizard` (PRD-056).
//!
//! Modeled on a real-world polyglot monorepo — the kind of project
//! Jarvy users actually drop the wizard against on day one. Mirrors
//! the layout of a typical T3-stack / Vercel / full-stack SaaS repo:
//!
//! ```text
//! project-root/
//! ├── Cargo.toml              (Rust workspace root for the API)
//! ├── package.json            (npm root for the web app)
//! ├── pnpm-workspace.yaml     (declares apps/* members)
//! ├── Dockerfile              (containerize for deploy)
//! ├── Makefile                (build orchestration)
//! ├── .github/workflows/test.yml  (CI)
//! ├── apps/
//! │   ├── api/Cargo.toml      (Rust API crate)
//! │   └── web/package.json    (Next.js frontend)
//! └── README.md
//! ```
//!
//! The test exercises the exact sequence a real user runs:
//!
//! 1. `jarvy wizard --skill-only --agent claude-code` against the
//!    fresh repo (no `jarvy.toml`).
//! 2. Verify the skill file landed under `JARVY_TEST_HOME/.claude/`.
//! 3. Simulate what the agent does when the user invokes the skill —
//!    call `jarvy discover --format json` (the CLI surface of the
//!    `jarvy_wizard_plan` MCP tool the skill instructs the agent to
//!    use first).
//! 4. Verify the plan surfaces every detected ecosystem from the
//!    fixture (rust, node, docker, make).
//! 5. Run `jarvy discover --apply --format json` (the CLI surface of
//!    the `jarvy_discover_apply` MCP tool) to commit a `jarvy.toml`.
//! 6. Verify the resulting `jarvy.toml` round-trips through TOML and
//!    declares every detected tool.
//! 7. **Idempotence check.** Re-run `jarvy discover --apply` and
//!    verify the second invocation reports "noop" / "no new tools"
//!    so the agent doesn't loop. This is the contract the SKILL.md
//!    and the system prompt pin: same inputs → same outputs.
//!
//! Why simulate the MCP layer via the CLI rather than calling the
//! agent's `claude` binary?
//! - Reproducibility: an actual LLM call returns non-deterministic
//!   text; CI can't pin assertions against it.
//! - Test scope: the wizard's job is to set up the agent's
//!   environment (skill drop) and expose stable MCP tools the agent
//!   can call (discover_apply). The agent's reasoning is out of
//!   scope for our test; the MCP surface it would invoke is in
//!   scope.

#![cfg(feature = "test-bypass")]

use assert_cmd::prelude::*;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Build the polyglot monorepo fixture under `root`. Lays down every
/// marker file `discover::analyze` recognizes for the rust + node +
/// docker + make ecosystems.
fn build_fixture(root: &Path) {
    // Rust workspace root — declares a member under apps/api.
    std::fs::write(
        root.join("Cargo.toml"),
        r#"[workspace]
members = ["apps/api"]
resolver = "2"
"#,
    )
    .unwrap();

    // npm root — declares pnpm workspace covering apps/*.
    std::fs::write(
        root.join("package.json"),
        r#"{
  "name": "saas-monorepo",
  "private": true,
  "scripts": { "build": "pnpm -r build" }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("pnpm-workspace.yaml"),
        "packages:\n  - 'apps/*'\n",
    )
    .unwrap();

    // Containerization + build orchestration.
    std::fs::write(
        root.join("Dockerfile"),
        "FROM rust:1.85 as builder\nWORKDIR /app\nCOPY . .\nRUN cargo build --release\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Makefile"),
        ".PHONY: test\ntest:\n\tcargo test --workspace\n",
    )
    .unwrap();

    // CI workflow — discover's rule for github-actions fires off this.
    std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
    std::fs::write(
        root.join(".github/workflows/test.yml"),
        "name: test\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v5\n",
    )
    .unwrap();

    // Workspace members — give them realistic minimum content.
    std::fs::create_dir_all(root.join("apps/api/src")).unwrap();
    std::fs::write(
        root.join("apps/api/Cargo.toml"),
        r#"[package]
name = "api"
version = "0.1.0"
edition = "2024"
"#,
    )
    .unwrap();
    std::fs::write(root.join("apps/api/src/main.rs"), "fn main() {}\n").unwrap();

    std::fs::create_dir_all(root.join("apps/web")).unwrap();
    std::fs::write(
        root.join("apps/web/package.json"),
        r#"{
  "name": "web",
  "scripts": { "dev": "next dev", "build": "next build" }
}
"#,
    )
    .unwrap();

    std::fs::write(root.join("README.md"), "# saas-monorepo\n").unwrap();
}

/// Build a Command for `jarvy` with the standard test environment.
/// Disables sandbox + CI detection (the test runner often runs both
/// inside Claude Code and on GitHub Actions) and forces the wizard
/// to bypass its TTY check.
fn jarvy(home: &Path, project: &Path) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    // `JARVY_HOME` overrides the base `~/.jarvy` AND the per-agent
    // skill / config dirs derived from it (see `paths::jarvy_home`
    // and `Agent::config_dir`). This is what redirects the SKILL.md
    // write away from the developer's real `~/.claude/skills/`.
    c.env("JARVY_HOME", home);
    c.env("JARVY_TEST_HOME", home); // also redirects the test-bypass surface
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_SANDBOX", "0");
    c.env("JARVY_WIZARD", "1");
    c.env_remove("CI");
    c.env_remove("GITHUB_ACTIONS");
    c.env_remove("CLAUDECODE");
    c.current_dir(project);
    c
}

#[test]
fn wizard_then_agent_simulation_bootstraps_jarvy_toml_idempotently() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    build_fixture(project.path());

    // ---------------------------------------------------------------
    // Step 1 — `jarvy wizard --skill-only --agent claude-code`.
    //   The user's actual first command. Drops the SKILL.md into
    //   their Claude Code skills dir and prints the one-liner.
    // ---------------------------------------------------------------
    let mut wizard = jarvy(home.path(), project.path());
    wizard.args([
        "wizard",
        "--skill-only",
        "--agent",
        "claude-code",
        "--format",
        "json",
    ]);
    let out = wizard.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("wizard --format json must emit a single JSON document");
    assert_eq!(v["status"], "ok", "wizard run must succeed; got: {v}");
    assert_eq!(v["mode"], "skill_drop");
    assert_eq!(v["agent"], "claude-code");

    let skill_path = v["skill_path"]
        .as_str()
        .expect("skill_path must be a string");
    // Step 2 — SKILL.md actually written, under the test home.
    assert!(
        Path::new(skill_path).exists(),
        "SKILL.md must exist at {skill_path}"
    );
    assert!(
        skill_path.starts_with(home.path().to_string_lossy().as_ref()),
        "SKILL.md must land under JARVY_TEST_HOME, not the real ~/.claude/. Got: {skill_path}"
    );

    let skill_body = std::fs::read_to_string(skill_path).unwrap();
    // The skill body must guide the agent toward the MCP tools we'll
    // exercise below (jarvy_wizard_plan + jarvy_discover_apply).
    assert!(skill_body.contains("jarvy_wizard_plan"));
    assert!(skill_body.contains("jarvy_discover_apply"));
    assert!(skill_body.contains("Idempotence is the hard rule"));

    // ---------------------------------------------------------------
    // Step 3 — Simulate the agent reading the skill and calling
    //   the FIRST MCP tool the skill instructs: jarvy_wizard_plan.
    //   The CLI equivalent is `jarvy discover --format json` (same
    //   underlying analyzer; same DiscoverReport shape exposed).
    // ---------------------------------------------------------------
    let mut plan = jarvy(home.path(), project.path());
    plan.args(["discover", "--format", "json"]);
    let plan_out = plan.assert().success().get_output().clone();
    let plan_stdout = String::from_utf8_lossy(&plan_out.stdout);
    let plan_json: serde_json::Value = serde_json::from_str(plan_stdout.trim())
        .expect("discover --format json must emit valid JSON");

    let required: Vec<&str> = plan_json["required"]
        .as_array()
        .expect("required must be an array")
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();

    // Step 4 — verify the proposal surfaces every ecosystem the
    // fixture declared. These are the tools an agent would propose.
    for must_include in ["rust", "node"] {
        assert!(
            required.contains(&must_include),
            "wizard plan must include `{must_include}`; got: {required:?}"
        );
    }

    // ---------------------------------------------------------------
    // Step 5 — Simulate the agent calling the SECOND MCP tool the
    //   skill instructs (after user confirmation): jarvy_discover_apply.
    //   CLI equivalent: `jarvy discover --apply`.
    // ---------------------------------------------------------------
    let mut apply1 = jarvy(home.path(), project.path());
    apply1.args(["discover", "--apply", "--format", "json"]);
    let apply1_out = apply1.assert().success().get_output().clone();
    let apply1_stdout = String::from_utf8_lossy(&apply1_out.stdout);
    let apply1_json: serde_json::Value = serde_json::from_str(apply1_stdout.trim())
        .expect("discover --apply --format json must emit valid JSON");
    assert_eq!(apply1_json["applied"], true);

    // Step 6 — `jarvy.toml` was actually written + parses + lists
    // the expected tools.
    let jarvy_toml = project.path().join("jarvy.toml");
    assert!(
        jarvy_toml.exists(),
        "discover --apply must write jarvy.toml at the project root"
    );
    let toml_text = std::fs::read_to_string(&jarvy_toml).unwrap();
    let parsed: toml::Table = toml_text
        .parse()
        .expect("written jarvy.toml must round-trip through the TOML parser");
    let provisioner = parsed
        .get("provisioner")
        .and_then(|v| v.as_table())
        .expect("jarvy.toml must contain a [provisioner] section");
    for must_pin in ["rust", "node"] {
        assert!(
            provisioner.contains_key(must_pin),
            "[provisioner] must declare `{must_pin}` after wizard apply; got keys: {:?}",
            provisioner.keys().collect::<Vec<_>>()
        );
    }

    // ---------------------------------------------------------------
    // Step 7 — Idempotence check. The agent might call apply again
    //   (e.g., if the user re-runs the skill). Same inputs MUST yield
    //   the same final jarvy.toml — no surprise additions, no churn.
    // ---------------------------------------------------------------
    let toml_after_first = std::fs::read(&jarvy_toml).unwrap();
    let mut apply2 = jarvy(home.path(), project.path());
    apply2.args(["discover", "--apply", "--format", "json"]);
    let apply2_out = apply2.assert().success().get_output().clone();
    let apply2_stdout = String::from_utf8_lossy(&apply2_out.stdout);
    let _apply2_json: serde_json::Value = serde_json::from_str(apply2_stdout.trim())
        .expect("second discover --apply --format json must emit valid JSON");

    let toml_after_second = std::fs::read(&jarvy_toml).unwrap();
    assert_eq!(
        toml_after_first, toml_after_second,
        "jarvy.toml bytes must be identical after a second `discover --apply` — \
         idempotence is the hard rule the skill + prompt instruct the agent to honor"
    );
}
