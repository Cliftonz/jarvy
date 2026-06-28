---
title: "Contributor testing guide — Jarvy"
description: "How to run, write, and structure tests when contributing to Jarvy. Unit tests, integration tests, snapshot fixtures, and the cross-platform E2E harness."
tags:
  - contributing
  - testing
---

# Contributor testing guide

Jarvy's test surface has three layers. Knowing which one to add to is the first decision when fixing a bug or shipping a feature.

| Layer | Lives in | Runtime | When to use |
|-------|----------|---------|-------------|
| **Unit** | `#[cfg(test)] mod tests { ... }` inside the module under test | < 50 ms | Pure logic: parsers, version comparisons, config merging, knob validators |
| **Integration** | `tests/*.rs` | < 5 s | Wire two modules together: parse a `jarvy.toml`, dispatch a command, check output |
| **E2E** | `tests/e2e_*.rs` + GitHub Actions matrix | seconds — minutes | Verify a real `jarvy setup` against real package managers on a real OS |

Always reach for the smallest layer that proves the bug doesn't come back. If a parser bug can be expressed as a unit test, don't write an integration test for it.

---

## Run the suite

```bash
cargo test                                              # everything
cargo test --lib                                        # unit tests only (~1.5s)
cargo test --tests                                      # integration tests only
cargo test --test cli_dispatch -- --show-output         # single integration file
cargo test --test cli_dispatch test_setup_dry_run       # single test by name
cargo nextest run                                       # faster runner if you have it
```

CI runs the same commands. If `cargo test` is green locally, CI will be green for that target.

---

## Required gates before committing

```bash
cargo fmt --all
cargo clippy --all-features -- -D warnings
cargo test
```

Pre-commit hooks aren't enforced, but the CI pipeline (`.github/workflows/test.yml` + `clippy.yml`) blocks the merge on any of these failing. Hooking them locally is the high-leverage move:

```bash
# .git/hooks/pre-commit
#!/usr/bin/env bash
set -e
cargo fmt --all --check
cargo clippy --all-features -- -D warnings
cargo test --lib
```

---

## Unit tests

Co-locate them with the code they test. The repo convention:

```rust
// src/version.rs

pub fn satisfies(requirement: &str, actual: &str) -> bool { ... }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_minor_accepts_patch_bump() {
        assert!(satisfies("^1.2.0", "1.2.5"));
        assert!(satisfies("^1.2.0", "1.3.0"));
        assert!(!satisfies("^1.2.0", "2.0.0"));
    }
}
```

**What to test at this layer:**

- Pure functions: version comparators, glob matchers, redactors
- TOML deserializers: every `#[derive(Deserialize)]` shape worth shipping should have a `test_parse_*` test that exercises the documented syntax
- Knob validators: every config struct has a destructure test against its `*_KNOBS` slice — see `src/packages/config.rs::tests` for the pattern

**Don't test from this layer:**

- Subprocess execution (brew install, dotnet, etc.) — those go in integration or E2E
- Filesystem mutations beyond `tempfile::tempdir()` scratch space
- Network calls (use `mockito` or push to E2E)

---

## Integration tests

Live in `tests/*.rs`, one file per concern. Common dependencies (already in `Cargo.toml`):

| Crate | Purpose |
|-------|---------|
| `assert_cmd` | Run the `jarvy` binary, assert exit code + stdout/stderr |
| `predicates` | Composable assertions (`predicate::str::contains(...)`) |
| `tempfile` | Scratch dirs + files |
| `insta` | Snapshot tests for human-readable output |

### Pattern: dispatch a command

```rust
// tests/cli_validate_bad_config.rs
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;
use std::io::Write;

#[test]
fn validate_rejects_flag_like_package_name() {
    let mut cfg = NamedTempFile::new().unwrap();
    writeln!(cfg, r#"
[provisioner]
git = "latest"

[npm]
"--registry=http://evil" = "1.0"
"#).unwrap();

    Command::cargo_bin("jarvy").unwrap()
        .args(["validate", "--file", cfg.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Refused [npm] entry"));
}
```

### Pattern: test mode (skip subprocess execution)

Setting `JARVY_TEST_MODE=1` disables interactive prompts. Setting `JARVY_FAST_TEST=1` short-circuits external command execution — tools claim to install instantly. Use these to exercise dispatch and reporting without spending real install time.

```rust
Command::cargo_bin("jarvy").unwrap()
    .args(["setup", "--file", "examples/minimal.toml", "--dry-run"])
    .env("JARVY_TEST_MODE", "1")
    .env("JARVY_FAST_TEST", "1")
    .assert()
    .success();
```

### Snapshot testing

For commands that produce structured human output (`jarvy tools`, `jarvy doctor`, `jarvy explain`), use `insta`:

```rust
let output = Command::cargo_bin("jarvy").unwrap()
    .args(["tools", "--index"])
    .env("JARVY_TEST_MODE", "1")
    .output().unwrap();
insta::assert_snapshot!(String::from_utf8_lossy(&output.stdout));
```

First run writes `*.snap`. Subsequent runs diff against it. `cargo insta review` to accept changes after intentional output edits.

### Fixtures

Shared test data lives in `tests/fixtures/`. The directory layout mirrors the test file:

```
tests/
  examples_validation.rs        # uses fixtures/examples_validation/*.toml
  fixtures/
    examples_validation/
      good.toml
      bad-syntax.toml
      flag-like.toml
```

Add a fixture instead of building it with `writeln!` when the input is long enough that the test signal gets lost in escaping.

---

## E2E tests

`tests/e2e_*.rs` exercise a real `jarvy` binary against real package managers. They are gated by `JARVY_E2E=1` so they don't slow down the default `cargo test`:

```bash
JARVY_E2E=1 cargo test --test e2e_base_tools -- --show-output
```

CI runs the matrix on every PR via `.github/workflows/e2e-cross-platform.yml` — macOS Intel + ARM, Ubuntu 22.04 + 24.04, Windows. Each runner builds Jarvy from the PR commit, then installs a base tool set (`git`, `jq`, `ripgrep`, `curl`, `wget`, `node`, `python`, `rust`, `go`) and asserts every binary lands on `PATH`.

**Add an E2E test when** a bug only reproduces with a real package manager — winget failing on a specific Windows build, brew picking the wrong tap, apt-get's noninteractive frontend swallowing a prompt. Those bugs do not surface in integration tests.

**Don't add an E2E test when** the symptom is covered by an integration test with `assert_cmd` and `JARVY_FAST_TEST=1`. E2E runs are slow and rate-limited.

See [e2e-testing-harness](e2e-testing-harness.md) for the architecture and platform matrix.

---

## Platform-specific tests

Use `#[cfg(target_os = "...")]` to gate tests that only make sense on one OS:

```rust
#[test]
#[cfg(target_os = "macos")]
fn macos_uses_brew_for_jq() {
    // brew-specific assertions
}

#[test]
#[cfg(target_os = "linux")]
fn linux_picks_apt_on_debian() { ... }

#[test]
#[cfg(target_os = "windows")]
fn windows_winget_id_format() { ... }
```

CI matrices verify each `cfg` branch on its native runner.

---

## What good test coverage looks like

When you add a `define_tool!` invocation for `mytool`:

1. Unit: a smoke test in `src/tools/mytool/definition.rs::tests` that calls `tool.expected_outcome_on_platform(Os::Mac)` etc. and asserts it returns `Outcome::Installable` (not `Outcome::Unsupported`). The `git` and `brew` definitions are the canonical examples.
2. Integration: only if the tool has unusual behavior — custom install script, complex dependency chain, post-install hook that touches the user's shell rc. A vanilla `brew install mytool` does NOT need an integration test.
3. E2E: only if the tool is in the base tool set tier (universal, every-platform). Most new tools never get E2E coverage and that's fine.

When you fix a bug:

1. Write the failing test FIRST. If you can't write it as a unit test, justify why before reaching for integration.
2. The test name should describe the bug, not the fix: `setup_reports_zero_exit_when_all_tools_already_installed`, not `test_setup`.

---

## Common pitfalls

- **Tests that depend on `~/.jarvy/config.toml`** — use `JARVY_HOME=<tempdir>` to isolate. The init module checks this env var before falling back to `$HOME/.jarvy`.
- **Tests that race the file logger** — call `tracing_subscriber::fmt::try_init()` exactly once per test process. The integration test harness handles this; per-test wiring is usually wrong.
- **`Command::cargo_bin` rebuilds the binary** — first call is slow (cargo build), subsequent calls reuse. Don't add `cargo build --release` to test setup unless you genuinely need release artifacts.
- **CI passing locally but failing in CI matrix** — almost always env pollution. CI runners are cleaner than your laptop. `env -i cargo test` reproduces.

---

## Next

- [Adding tools](adding-tools.md) — the `define_tool!` macro contract
- [Architecture](architecture.md) — module map, where things live
- [E2E testing harness](e2e-testing-harness.md) — cross-platform matrix details
- [Contributing](contributing.md) — repo workflow, commit style, PR process
