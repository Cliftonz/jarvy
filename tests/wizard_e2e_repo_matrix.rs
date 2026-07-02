//! End-to-end repo-shape matrix for `jarvy wizard` / `jarvy discover`.
//!
//! The original wizard happy-path test (`wizard_e2e_happy_path.rs`)
//! covers exactly one repo layout: Rust + Node + Docker + Make in a
//! T3-style monorepo. That test exercises the full skill-drop
//! interaction — SKILL.md landing under the right agent dir,
//! idempotent re-apply, JSON output shape — but only against one
//! ecosystem combination.
//!
//! This matrix test covers the repo *shapes* Jarvy is expected to
//! bootstrap. Each row is one fixture layout + the tools the wizard
//! must surface as `required` (own-marker present) and `recommended`
//! (companion of a detected tool). Because the wizard's real job on
//! `--apply` collapses to `jarvy_discover_apply` + `jarvy_validate_
//! config`, we skip the skill-drop half here — it's already pinned
//! in the happy-path test — and drive `jarvy discover --apply`
//! directly, which is the exact CLI surface the wizard's MCP tool
//! wraps.
//!
//! Adding a new repo shape is one row in `matrix()` — no per-shape
//! `#[test]` boilerplate.

#![cfg(feature = "test-bypass")]

use assert_cmd::prelude::*;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// One repo shape under test.
struct RepoShape {
    /// Human-readable label surfaced in assertion failures.
    name: &'static str,
    /// Files to touch at the fixture root before running discover.
    /// Empty content is fine — the rules match on filename presence,
    /// not content (Cargo.toml is the one exception: needs `[package]`
    /// for scanner sanity but we pass a minimal valid TOML in
    /// `write_marker`).
    markers: &'static [&'static str],
    /// Directories to create (for markers like `.git`, `.github`).
    dirs: &'static [&'static str],
    /// Tools that MUST appear under `required` in the discover report.
    /// A shape without every named tool fails the test.
    required: &'static [&'static str],
    /// Tools that MUST appear under `recommended`. Recommended-only
    /// (companion) suggestions belong here — required tools are asked
    /// for in `required` above.
    recommended: &'static [&'static str],
    /// Minimum `warning_count` this shape's discover output MUST
    /// produce when passed through `jarvy validate`. `0` (the default
    /// for shapes with no advisory-signal expectation) simply
    /// tolerates any warnings; `>0` guards against a regression that
    /// silently drops the warning emitter — e.g., the polyglot shape
    /// intentionally warns about node-without-nvm, and a refactor
    /// that stops emitting that warning would be invisible without
    /// this pin.
    min_warnings: u64,
}

/// Registry of repo shapes. Add a row here to grow coverage.
fn matrix() -> Vec<RepoShape> {
    vec![
        // Full 4-lang polyglot with lockfile-specific PM detection.
        // Pins the fix from the "duplicate release-plz key" era +
        // lockfile-precision claim (pnpm-lock → require pnpm, not
        // suggest [pnpm, yarn]).
        RepoShape {
            name: "polyglot_node_php_rust_go",
            markers: &[
                "Cargo.toml",
                "package.json",
                "pnpm-lock.yaml",
                "composer.json",
                "go.mod",
            ],
            dirs: &[],
            required: &["rust", "node", "pnpm", "php", "composer", "go"],
            recommended: &["bacon", "cargo-nextest", "golangci-lint", "air", "delve"],
            // The polyglot shape has `node` present without `nvm` in
            // `[provisioner]`, so `jarvy validate` emits an advisory
            // warning per its convention. Pin `>= 1` so a refactor
            // that stops emitting the advisory trips this test
            // instead of silently regressing the operator signal.
            min_warnings: 1,
        },
        // Yarn-Berry repo — pin the yarn.lock → yarn required path
        // and the negative case that pnpm doesn't sneak in.
        RepoShape {
            name: "yarn_workspace",
            markers: &["package.json", "yarn.lock"],
            dirs: &[],
            required: &["node", "yarn"],
            recommended: &[], min_warnings: 0,
        },
        // Rust project on GitHub with release-plz automation. Pins
        // the .github/ + release-plz.toml + .git combo without
        // duplicating release-plz between required + recommended
        // (regression guard on the analyze_with dedup).
        RepoShape {
            name: "rust_with_release_plz_on_github",
            markers: &["Cargo.toml", "release-plz.toml"],
            dirs: &[".git", ".github"],
            required: &["rust", "git", "gh", "release-plz"],
            recommended: &["bacon", "cargo-nextest"], min_warnings: 0,
        },
        // Laravel PHP project — artisan + composer.json is the
        // canonical marker set. Verifies php + composer both required.
        RepoShape {
            name: "laravel_php",
            markers: &["composer.json", "composer.lock", "artisan"],
            dirs: &[],
            required: &["php", "composer"],
            recommended: &[], min_warnings: 0,
        },
        // Bun-first Node project — bun.lockb elevates bun to required.
        RepoShape {
            name: "bun_project",
            markers: &["package.json", "bun.lockb"],
            dirs: &[],
            required: &["node", "bun"],
            recommended: &[], min_warnings: 0,
        },
        // BEAM ecosystem — Elixir + Gleam interop, both recommend
        // Erlang as a runtime companion. Guards the elixir + gleam
        // recommends-erlang wiring.
        RepoShape {
            name: "beam_elixir_gleam",
            markers: &["mix.exs", "gleam.toml"],
            dirs: &[],
            required: &["elixir", "gleam"],
            recommended: &["erlang"], min_warnings: 0,
        },
        // Haskell + Deno + Zig — three of the niche langs added in
        // the audit pass. Purely about "detected + required" wiring;
        // none has companion suggestions.
        RepoShape {
            name: "niche_haskell_deno_zig",
            markers: &["cabal.project", "deno.json", "build.zig"],
            dirs: &[],
            required: &["haskell", "deno", "zig"],
            recommended: &[], min_warnings: 0,
        },
        // OCaml + Nim + Crystal — another niche cluster.
        RepoShape {
            name: "niche_ocaml_nim_crystal",
            markers: &["dune-project", "hello.nimble", "shard.yml"],
            dirs: &[],
            required: &["ocaml", "nim", "crystal"],
            recommended: &[], min_warnings: 0,
        },
        // Bazel monorepo — MODULE.bazel (bzlmod) is the modern marker.
        RepoShape {
            name: "bazel_monorepo",
            markers: &["MODULE.bazel", ".bazelversion"],
            dirs: &[],
            required: &["bazelisk"],
            recommended: &[], min_warnings: 0,
        },
        // K8s-native project driven by Skaffold + Kustomize + Helm.
        // Verifies skaffold.yaml + k8s/ dir + Chart.yaml simultaneously.
        RepoShape {
            name: "k8s_skaffold_dev_loop",
            markers: &["skaffold.yaml", "Chart.yaml"],
            dirs: &["k8s"],
            required: &["skaffold", "kubectl", "helm"],
            recommended: &["kustomize", "k9s"], min_warnings: 0,
        },
        // C/C++ project with CMake + Docker.
        RepoShape {
            name: "cmake_containerized",
            markers: &["CMakeLists.txt", "Dockerfile"],
            dirs: &[],
            required: &["cmake", "docker"],
            recommended: &[], min_warnings: 0,
        },
        // Python + Infisical + VSCode devcontainer. Verifies the
        // secret-manager + editor combo landing simultaneously.
        RepoShape {
            name: "python_with_secrets_and_vscode",
            markers: &["pyproject.toml", ".infisical.json"],
            dirs: &[".vscode"],
            required: &["python", "infisical", "vscode"],
            recommended: &[], min_warnings: 0,
        },
    ]
}

fn write_marker(root: &Path, name: &str) {
    // Cargo.toml is the one file discover doesn't just presence-check
    // — some validation paths later touch it. Give it minimal valid
    // content; every other marker can be empty.
    let content = match name {
        "Cargo.toml" => "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        "package.json" => r#"{"name": "fixture", "private": true}"#,
        "composer.json" => r#"{"name": "fixture/app"}"#,
        "go.mod" => "module example.com/fixture\ngo 1.22\n",
        _ => "",
    };
    std::fs::write(root.join(name), content).unwrap();
}

fn jarvy(home: &Path, project: &Path) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_HOME", home);
    c.env("JARVY_TEST_HOME", home);
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

/// Drive one shape through `jarvy discover --apply --format json` and
/// verify the report + resulting `jarvy.toml`. Returns nothing —
/// panics with a descriptive message on any mismatch so the caller
/// can iterate cheaply.
fn assert_shape(shape: &RepoShape) {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    for dir in shape.dirs {
        std::fs::create_dir_all(project.path().join(dir)).unwrap();
    }
    for marker in shape.markers {
        write_marker(project.path(), marker);
    }

    // Preview first — matches what the wizard's `jarvy_wizard_plan`
    // MCP tool returns before any mutation. Verifying preview + apply
    // separately catches drift between the two code paths.
    let mut preview = jarvy(home.path(), project.path());
    preview.args(["discover", "--format", "json"]);
    let preview_stdout = String::from_utf8_lossy(
        &preview
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .to_string();
    let preview_json: serde_json::Value = serde_json::from_str(preview_stdout.trim())
        .unwrap_or_else(|e| panic!("[{}] preview must emit JSON: {e}", shape.name));

    let preview_required: Vec<String> = preview_json["required"]
        .as_array()
        .expect("required must be an array")
        .iter()
        .filter_map(|s| s["name"].as_str().map(str::to_string))
        .collect();
    for want in shape.required {
        assert!(
            preview_required.iter().any(|s| s == want),
            "[{}] preview required missing `{want}`; got: {preview_required:?}",
            shape.name
        );
    }
    let preview_recommended: Vec<String> = preview_json["recommended"]
        .as_array()
        .expect("recommended must be an array")
        .iter()
        .filter_map(|s| s["name"].as_str().map(str::to_string))
        .collect();
    for want in shape.recommended {
        assert!(
            preview_recommended.iter().any(|s| s == want),
            "[{}] preview recommended missing `{want}`; got: {preview_recommended:?}",
            shape.name
        );
    }

    // Apply — the wizard's `jarvy_discover_apply` MCP surface.
    let mut apply = jarvy(home.path(), project.path());
    apply.args(["discover", "--apply", "--format", "json"]);
    let apply_stdout = String::from_utf8_lossy(
        &apply.assert().success().get_output().stdout.clone(),
    )
    .to_string();
    let apply_json: serde_json::Value = serde_json::from_str(apply_stdout.trim())
        .unwrap_or_else(|e| panic!("[{}] apply must emit JSON: {e}", shape.name));
    assert_eq!(
        apply_json["applied"], true,
        "[{}] apply must report applied=true; got: {apply_json}",
        shape.name
    );

    // jarvy.toml lands + round-trips through the TOML parser (guards
    // against the duplicate-key regression the polyglot fixture
    // originally shipped with).
    let jarvy_toml = project.path().join("jarvy.toml");
    assert!(
        jarvy_toml.exists(),
        "[{}] jarvy.toml must exist after apply",
        shape.name
    );
    let toml_text = std::fs::read_to_string(&jarvy_toml).unwrap();
    let parsed: toml::Table = toml_text.parse().unwrap_or_else(|e| {
        panic!(
            "[{}] jarvy.toml must round-trip through TOML parser; \
             a duplicate `[provisioner]` key would fail here. \
             Contents:\n{toml_text}\nerror: {e}",
            shape.name
        )
    });
    let provisioner = parsed
        .get("provisioner")
        .and_then(|v| v.as_table())
        .unwrap_or_else(|| {
            panic!(
                "[{}] jarvy.toml must contain [provisioner]. Contents:\n{toml_text}",
                shape.name
            )
        });
    for want in shape.required {
        assert!(
            provisioner.contains_key(*want),
            "[{}] [provisioner] must declare `{want}` after apply; got keys: {:?}",
            shape.name,
            provisioner.keys().collect::<Vec<_>>()
        );
    }

    // File perms: 0644 (see the perms fix in
    // `discover::commands::atomic_write`). Guards against
    // NamedTempFile's 0600 secure default leaking through.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&jarvy_toml).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o644,
            "[{}] jarvy.toml must be 0644 (repo config, readable to \
             collaborators after `git clone`); got {mode:o}",
            shape.name
        );
    }

    // `jarvy validate` must accept the discover-emitted config.
    // `toml::Table::parse` alone accepts any well-formed TOML, but
    // `jarvy validate` runs the `validate_package_name` /
    // `validate_package_version` guardrails, the `TOP_LEVEL_SECTIONS`
    // schema check, and the remote-config trust-boundary logic — much
    // stronger evidence that a regression didn't sneak in a name with
    // a control byte, a mis-aliased dash/underscore, or a version
    // string that fails our sanitisation.
    //
    // Tolerate advisory warnings (exit 1) as long as `valid=true`.
    // The polyglot Node+PHP+Rust+Go shape emits a "node requires nvm"
    // warning by design — that's advisory guidance, not a hard error;
    // failing the assertion would gate CI on an intentional warning.
    // Only `error_count > 0` (exit 2) is a real regression signal.
    let mut validate = jarvy(home.path(), project.path());
    validate.args(["validate", "--format", "json"]);
    let output = validate.output().expect("validate must spawn");
    let validate_stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert_ne!(
        output.status.code(),
        Some(2),
        "[{}] jarvy validate returned CONFIG_ERROR (exit 2) — discover \
         emitted a config that fails schema validation. stdout:\n{validate_stdout}",
        shape.name
    );
    let validate_json: serde_json::Value = serde_json::from_str(validate_stdout.trim())
        .unwrap_or_else(|e| {
            panic!("[{}] jarvy validate must emit JSON: {e}", shape.name)
        });
    assert_eq!(
        validate_json["valid"], true,
        "[{}] jarvy validate must report valid=true; got: {validate_json}",
        shape.name
    );
    assert_eq!(
        validate_json["error_count"],
        serde_json::json!(0),
        "[{}] validate error_count must be zero (warnings OK); got: {validate_json}",
        shape.name
    );
    // QA F8: enforce the shape's `min_warnings` contract. Shapes
    // that intentionally emit advisories (e.g. polyglot node-without-nvm)
    // set min_warnings ≥ 1 so a refactor stopping the emission trips
    // this test rather than silently dropping the operator signal.
    let warn_count = validate_json["warning_count"].as_u64().unwrap_or(0);
    assert!(
        warn_count >= shape.min_warnings,
        "[{}] validate warning_count must be ≥ {} (advisory-emission \
         guard); got {}. Refactor may have silently dropped the warning \
         path this shape exists to exercise.",
        shape.name,
        shape.min_warnings,
        warn_count
    );

    // Idempotence — a second apply must produce byte-identical output.
    // Matches the contract the SKILL.md pins for wizard reruns.
    let bytes_before = std::fs::read(&jarvy_toml).unwrap();
    let mut apply2 = jarvy(home.path(), project.path());
    apply2.args(["discover", "--apply", "--format", "json"]);
    apply2.assert().success();
    let bytes_after = std::fs::read(&jarvy_toml).unwrap();
    assert_eq!(
        bytes_before, bytes_after,
        "[{}] second apply must be a byte-for-byte no-op",
        shape.name
    );
}

/// Negative shape: an empty repo (no language markers, no infra
/// markers) must produce a report with zero required tools and — per
/// the discover apply logic — no jarvy.toml write, exit 0. Guards
/// against a regression where a spurious catch-all rule (or a walk
/// that leaks into `.git/` / vendored deps) surfaces detections in
/// a truly-empty project. Also validates that the wizard's "step 2 —
/// stop if project is already configured" no-op branch behaves
/// correctly when the plan is empty.
#[test]
fn wizard_discover_over_empty_repo_writes_nothing() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    // Deliberately no markers, no dirs, no `.git`.

    let mut preview = jarvy(home.path(), project.path());
    preview.args(["discover", "--format", "json"]);
    let stdout = String::from_utf8_lossy(
        &preview.assert().success().get_output().stdout.clone(),
    )
    .to_string();
    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("preview must emit JSON");
    let required = json["required"].as_array().expect("required array");
    assert!(
        required.is_empty(),
        "empty repo must have zero required tools; got {required:?}"
    );

    // Apply against an empty repo — behavior is either "no file written"
    // or "file written but empty [provisioner]". Either is acceptable
    // per the current UX; assert only that the exit is clean and, if
    // the file does exist, it round-trips through TOML.
    let mut apply = jarvy(home.path(), project.path());
    apply.args(["discover", "--apply", "--format", "json"]);
    apply.assert().success();
    let jarvy_toml = project.path().join("jarvy.toml");
    if jarvy_toml.exists() {
        let text = std::fs::read_to_string(&jarvy_toml).unwrap();
        let parsed: toml::Table = text
            .parse()
            .expect("if written, jarvy.toml must round-trip through TOML");
        let provisioner_empty = parsed
            .get("provisioner")
            .and_then(|v| v.as_table())
            .is_none_or(|t| t.is_empty());
        assert!(
            provisioner_empty,
            "empty repo apply must write no [provisioner] entries; got:\n{text}"
        );
    }
}

#[test]
fn wizard_discover_apply_over_repo_matrix() {
    for shape in matrix() {
        assert_shape(&shape);
    }
}
