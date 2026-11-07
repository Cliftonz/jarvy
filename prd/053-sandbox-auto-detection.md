# PRD-053: Sandbox Auto-Detection & Seamless Mode

## Overview

Jarvy already auto-disables telemetry and update checks on `CI=true`
(see `src/ci/mod.rs:322`, `src/telemetry.rs:132`,
`src/update/config.rs:221`). The CI heuristic does not cover modern AI
coding sandboxes — Claude Code, Cursor background agents, Devin,
OpenAI Codex, e2b, Modal, Daytona, Replit Agent — nor the
long-running container family (devcontainers, GitHub Codespaces,
Gitpod). Operators currently have to remember to set
`JARVY_TEST_MODE=1`, `CI=true`, `JARVY_TELEMETRY=0`, run
`jarvy drift accept --quiet` at image-build time, and fall back to
`jarvy doctor` when installs aren't allowed.

This PRD adds a single `crate::sandbox` detector that subsumes those
rituals and switches Jarvy into a **seamless mode**: quiet,
non-interactive, telemetry-off-by-default, auto-baselined,
verify-only when installs are impossible.

## Problem Statement

AI agents now spin up disposable sandboxes the way humans spin up
shells. Each sandbox surface — Claude Code's exec env, Cursor's
background agents, e2b boxes, Codespaces, devcontainers — has its
own conventions for non-interactivity, restricted filesystems, and
limited egress. Today, every operator who wants Jarvy to behave well
inside one of these has to:

1. Manually set `JARVY_TEST_MODE=1` so prompts don't hang the agent.
2. Manually set `CI=true` (or `JARVY_TELEMETRY=0`) so telemetry
   doesn't leak across tenants in shared base images.
3. Manually set `JARVY_UPDATE=0` so update checks don't fire on every
   container start.
4. Manually run `jarvy drift accept --quiet` at image bake time so a
   pre-loaded image has a drift baseline.
5. Manually decide whether the sandbox allows installs and switch to
   `jarvy doctor` if not.

That is five env vars and two commands to remember per sandbox
template. The result is sandboxes that hang on prompts, leak
telemetry between tenants, spam stderr with permission-denied lines,
and fail the agent's session with no useful diagnostic.

The recommendation in `docs/ai-sandboxes.md` already documents the
manual rituals. This PRD makes them unnecessary.

## Evidence

- `docs/ai-sandboxes.md` (added in the same PR series as this PRD)
  lists six manual environment knobs that all need to be set
  together to get reasonable behavior.
- `src/telemetry.rs:132` and `src/update/config.rs:221` already
  carry CI-aware special cases — proof that the underlying need is
  real, just narrower than it should be.
- `src/onboarding/detection.rs:65` re-implements its own CI check
  for first-run logic. That re-implementation will drift if the
  canonical CI list grows. Same drift risk applies to sandbox
  detection if it doesn't get its own module.
- Real-world failure mode (2026-05-12): `predev` hooks in a
  Codespace hung waiting on interactive prompts because nobody set
  `JARVY_TEST_MODE=1`. See `docs/for-ai-agents.md:170`.

## Requirements

### Functional Requirements

1. **Sandbox detection**: Detect Codespaces, Gitpod, devcontainers,
   Replit, e2b, Modal, Daytona, Claude Code, Cursor background
   agents, and a generic-container fallback (`/.dockerenv` present +
   non-TTY).
2. **Seamless mode**: When a sandbox is detected, default to
   non-interactive behavior:
   - Suppress prompts (implicit `JARVY_TEST_MODE=1` semantics).
   - Default to `--quiet` output.
   - Disable telemetry unless explicitly opted in.
   - Disable update checks unless explicitly opted in.
3. **Auto-baseline**: On the first run inside a sandbox, if
   `.jarvy/state.json` is absent **and** `jarvy doctor` shows zero
   gaps against the loaded `jarvy.toml`, silently snapshot the
   current state as the drift baseline. Never auto-baseline on a
   partial match.
4. **Verify-only fallback**: If the sandbox cannot install (read-only
   rootfs, OR no sudo + no userspace package manager on PATH), do
   not attempt installs. Run the doctor pipeline and exit non-zero
   if gaps exist.
5. **Single source of truth**: All callers (telemetry, update,
   onboarding, setup, services) consume the new module's
   `is_sandbox()` / `is_seamless()` / `detect()` API. The existing
   `src/update/config.rs:278` `is_ci_environment()` shim becomes
   `is_unattended()` and routes through the new module.
6. **Override**: `JARVY_SANDBOX=0` disables sandbox detection;
   `JARVY_SANDBOX=1` forces it. Same shape as `JARVY_NO_CI`/
   `JARVY_CI`.
7. **Visible banner**: On detection, print a single line to stderr:
   `[jarvy] detected <sandbox> — seamless mode (override: JARVY_SANDBOX=0)`.
   Suppressed if `--quiet` is also explicit on the CLI.

### Non-Functional Requirements

1. **Cost of detection < 1ms**: Pure environment-variable + stat
   reads. No subprocess fork.
2. **No false positives in a normal shell**: A developer running
   `jarvy setup` on their laptop must never trip the detector. The
   generic-container fallback gates on `/.dockerenv` AND non-TTY,
   both required.
3. **Backwards-compatible**: Existing CI behavior unchanged. CI is a
   subset of "seamless"; nothing already working in CI degrades.
4. **Testable**: Env-isolated unit tests modeled on
   `src/ci/mod.rs:332` so CI runners don't leak their own vars into
   detector tests.

## Non-Goals

- **Sandbox-specific install backends.** This PRD does not teach
  Jarvy to install via e2b's filesystem API or Modal's mount layer.
  It only decides whether to attempt installs at all.
- **Per-sandbox telemetry endpoints.** Telemetry stays off by default
  in seamless mode. Operators who want session-scoped telemetry
  still set `JARVY_OTLP_ENDPOINT` explicitly — this PRD does not
  introduce a sandbox-keyed endpoint registry.
- **Auto-detection of which jarvy.toml to load.** Sandbox detection
  changes *behavior*, not *configuration source*. Project config
  still comes from the same `--file` / `--from` paths as today.
- **Restricting which tools are installable in a sandbox.** Verify-
  only fallback is reactive (install attempt would fail) not
  policy-driven.

## Feature Specifications

### 1. Detection signals

| Sandbox | Primary signal | Secondary |
|---|---|---|
| GitHub Codespaces | `CODESPACES=true` | `CODESPACE_NAME` set |
| Gitpod | `GITPOD_WORKSPACE_ID` set | `GITPOD_*` family |
| Devcontainers (VS Code Remote Containers) | `REMOTE_CONTAINERS=true` | `DEVCONTAINER` set |
| Replit Agent | `REPL_ID` set | `REPLIT_USER` |
| e2b | `E2B_SANDBOX_ID` set | `E2B_API_KEY` set |
| Modal | `MODAL_TASK_ID` set | `MODAL_IS_REMOTE` |
| Daytona | `DAYTONA_WS_ID` set | `DAYTONA_WORKSPACE_DIR` |
| Claude Code | `CLAUDECODE=1` OR `CLAUDE_CODE_ENTRYPOINT` set | — |
| Cursor agent | `CURSOR_AGENT=1` | — |
| Generic container fallback | `/.dockerenv` exists | AND stdin is not a TTY |

Detection order: explicit override (`JARVY_SANDBOX=0/1`) > named
provider (in table order) > generic-container fallback > none.

Named providers are checked before CI providers because the named
sandbox signal is more specific (a Codespace also sets `CI=true`).

### 2. Public API

```rust
// src/sandbox/mod.rs

pub enum SandboxProvider {
    Codespaces,
    Gitpod,
    Devcontainer,
    Replit,
    E2b,
    Modal,
    Daytona,
    ClaudeCode,
    Cursor,
    GenericContainer,
}

pub struct SandboxEnvironment {
    pub provider: SandboxProvider,
    pub forced: bool,
    pub install_capable: InstallCapability,
}

pub enum InstallCapability {
    Full,                         // Probe passed: sudo or pkg manager available
    VerifyOnly(VerifyOnlyReason), // Read-only rootfs or no install path; reason carried for logs
}

pub enum VerifyOnlyReason {
    NoJarvyHome,     // ~/.jarvy/ could not be resolved
    ReadOnlyRoot,    // could not write into ~/.jarvy/
    NoInstallPath,   // no user-scope pkg manager AND `sudo -n true` fails
    Forced,          // JARVY_FORCE_VERIFY_ONLY=1 test override
}

// Note: the original PRD draft listed an `Unknown` variant "for
// dry-run". In practice the probe is fork-free in the happy path and
// only `sudo -n true` forks once per process, so we always probe and
// pick a concrete value. Dropped to keep the call sites pattern-
// exhaustive without a third arm.

pub fn detect() -> Option<SandboxEnvironment>;
pub fn is_sandbox() -> bool;

/// `is_seamless()` returns true when *either* CI *or* sandbox is
/// detected. This is the canonical "should I avoid prompts /
/// telemetry / update checks" predicate. All non-CI callers should
/// migrate to this.
pub fn is_seamless() -> bool;
```

### 3. Install-capability probe

Runs once per process at first call to `detect()`, result cached
in a `OnceLock<InstallCapability>`. Probe steps:

1. Try to create + delete a 0-byte file at `~/.jarvy/.probe`. If
   that fails with EROFS → `VerifyOnly`.
2. Check `sudo -n true` exits zero (passwordless sudo available)
   OR the platform's primary user-scope package manager is on PATH
   (`brew` on macOS, `cargo` always, `apt-get`/`dnf`/`pacman` won't
   help without sudo so they don't count). If neither → `VerifyOnly`.
3. Otherwise → `Full`.

`setup_cmd::run_setup` consults the cached capability; on
`VerifyOnly` it skips the install phase and runs the doctor pipeline
inline, exiting with `error_codes::PREREQ_MISSING` if any gaps.

### 4. Wiring

| Caller | Today | After PRD |
|---|---|---|
| `src/telemetry.rs:132` (CI auto-disable) | `if env::var("CI").is_ok() \|\| env::var("GITHUB_ACTIONS").is_ok()` | `if sandbox::is_seamless()` |
| `src/update/config.rs:221` | `if is_ci_environment() && env::var("JARVY_UPDATE").is_err()` | `if sandbox::is_seamless() && env::var("JARVY_UPDATE").is_err()` |
| `src/onboarding/detection.rs:18` | `if is_ci_environment()` | `if sandbox::is_seamless()` |
| `src/commands/setup_cmd.rs` (banner + quiet default) | always logs interactive banner | banner suppressed in seamless mode; auto-baseline path activates |
| `src/commands/setup_cmd.rs:197` (unknown-tool telemetry nag) | unconditional | suppressed in seamless mode (tenant noise) |

### 5. Auto-baseline behavior

Auto-baseline runs at two points in `setup_cmd::run_setup`:

**A. End-of-run path** (install-capable sandbox or CI):

```text
if sandbox::is_seamless()
   && !state_file_exists(".jarvy/state.json")
   && version_check.needs_install.is_empty()
   && version_check.unknown.is_empty()
then
   capture_drift_baseline(auto=true)
```

**B. Verify-only success path** (sandbox can't install but
version_check came back clean):

```text
if sandbox::is_seamless()
   && install_capability() == VerifyOnly
   && version_check.needs_install.is_empty()
   && version_check.unknown.is_empty()
   && !state_file_exists(".jarvy/state.json")
then
   capture_drift_baseline(auto=true)
   exit EXIT_SUCCESS with verify-only message
```

The second path is critical for the pre-loaded sandbox image use
case: a read-only container with all tools pre-baked still gets a
baseline written, so subsequent sessions can do meaningful drift
checks even though the container itself never runs an install.

Both paths share `capture_drift_baseline()` in
`src/commands/setup_cmd.rs` and write the same `.jarvy/state.json`
shape as the explicit `jarvy drift accept` command.

### 6. Verify-only fallback shape

`setup_cmd::run_setup` early in its body:

```text
match sandbox::detect() {
    Some(env) if env.install_capable == InstallCapability::VerifyOnly => {
        return commands::doctor::run_doctor(/* json=ci-mode */)
            .map(|gaps| if gaps.is_empty() { 0 } else {
                error_codes::PREREQ_MISSING
            });
    }
    _ => { /* normal path */ }
}
```

### 7. Banner format

Single line to stderr, only on first sandbox detection within a
process and only if no `--quiet` flag is set:

```
[jarvy] detected GitHub Codespaces — seamless mode active
        telemetry: off (set JARVY_TELEMETRY=1 to enable)
        prompts:   off  updates: off
        override:  JARVY_SANDBOX=0 jarvy <command>
```

(One physical newline between fields; indentation aligns with the
prefix length.)

## Acceptance Criteria

1. **Detector module exists** at `src/sandbox/` with `mod.rs` and
   a test module modeled on `src/ci/mod.rs:332`.
2. **Detection table** covers every row in section 1, with unit
   tests that env-isolate to avoid CI-runner leakage.
3. **`is_seamless()` replaces three callers**: `telemetry.rs:132`,
   `update/config.rs:221`, `onboarding/detection.rs:18`. The shim
   `update/config.rs:278::is_ci_environment` continues to exist for
   external library consumers but routes through `is_seamless()`.
4. **Auto-baseline lands in `.jarvy/state.json`** on a clean
   `setup_cmd` run in seamless mode when the doctor pipeline shows
   zero gaps. Verified by an integration test in
   `tests/sandbox_integration.rs` that boots a real debian:bookworm
   container (git pre-installed) with `JARVY_SANDBOX=1`, asserts
   the state file is written.
5. **Verify-only fallback** activates when a read-only root or
   no-install path is detected. Verified by integration tests
   (`tests/sandbox_integration.rs`) that boot debian:bookworm-slim
   (no git) with `JARVY_FORCE_VERIFY_ONLY=1` and assert exit
   code 3, and the same on debian:bookworm with git pre-installed
   and assert exit code 0.
6. **Banner**: one stderr line on first detect per process, gated
   on `!--quiet`. Verified by the
   `generic_container_emits_seamless_banner` integration test.
7. **No regression**: existing CI tests in `src/ci/mod.rs` pass
   unchanged. The CI detector remains the source of truth for
   provider-specific output formatting (log groups, output vars);
   only the *unattended-mode* semantics migrate.
8. **`cargo fmt --all`, `cargo clippy --all-features -- -D
   warnings`, `cargo check`, `cargo test --verbose`** all pass.

## Test Harness Notes

The integration suite in `tests/sandbox_integration.rs` spins real
Docker containers via the `docker` CLI (`DockerContainer` in
`tests/common/mod.rs`).
The harness is cross-platform:

- **Linux CI (ubuntu-latest)**: `cargo test` exercises the suite
  directly. Cargo's own host-built jarvy is already a Linux ELF,
  so it mounts into the container and runs natively.
- **macOS / Apple Silicon dev**: `make test-sandbox` cross-compiles
  jarvy to `aarch64-unknown-linux-gnu` via the `cross` Docker-based
  toolchain, then runs the harness with `JARVY_TEST_BIN` pointing at
  the cross-built binary. Apple Silicon hosts run `linux/arm64`
  debian containers natively under Docker Desktop — no QEMU
  emulation, full speed.
- **Skip behavior**: tests print a reason and return cleanly when
  Docker is unreachable OR when the resolved binary is not a Linux
  ELF. A stray `cargo test` on macOS without the cross setup is a
  clean skip, not a failure.

Each test:
- Mounts the binary at `/usr/local/bin/jarvy`
- Mounts a temp dir holding `jarvy.toml` at `/workspace`
- Sets `JARVY_HOME=/tmp/.jarvy` and `HOME=/tmp` to keep stateful
  paths on a writable tmpfs path inside the container
- Execs `jarvy setup` (or `setup --dry-run` for the banner test)
- Drains stdout/stderr, asserts exit code + specific stderr line

## Out-of-scope but tracked

- A `jarvy sandbox` subcommand for diagnostic ("what did Jarvy
  detect, why is it in seamless mode") — useful, but not gating
  on this PRD. File a follow-up if it doesn't make the first cut.
- Per-sandbox tool blocklists (e.g. "Claude Code sandbox should
  never try to install `docker`"). Adjacent concern; lives in roles
  (PRD-033), not here.
- Auto-detection of the right `--role` based on which sandbox is
  active. Tempting and dangerous — defer until there's user demand.

## Migration / compatibility

- **No new TOML schema.** Sandbox detection is environmental, not
  configured.
- **No removed env vars.** `JARVY_TEST_MODE`, `CI`, `JARVY_NO_CI`
  continue to work. They become redundant inside detected
  sandboxes but stay as explicit overrides.
- **`is_ci_environment()` in `src/update/config.rs:278`** keeps its
  name (other crates may import it) but internally calls
  `crate::sandbox::is_seamless()`. Rename deferred to a separate
  PR.

## Risks

| Risk | Mitigation |
|---|---|
| False positive: detector trips in a container the operator owns and wants full setup in | `JARVY_SANDBOX=0` escape hatch printed in the banner |
| Auto-baseline papers over a bug (wrong version present, jarvy.toml says different) | Only baseline on **full** doctor match; partial match → no auto-baseline |
| Sandbox env-var list rots as new products ship | Single detection file, mirror of `src/ci/mod.rs`; one-PR change to add a row |
| Detection-probe writes a file every run | Probe path is `~/.jarvy/.probe`, 0 bytes, deleted in the same call; cached per-process so it runs once |
| Banner adds noise to JSON-mode consumers | Banner is stderr-only and gated on `!--quiet`; JSON output on stdout unchanged |

## Implementation Plan

1. **`src/sandbox/mod.rs`** — detector + provider enum + tests.
2. **`src/sandbox/capability.rs`** — install-capability probe with
   `OnceLock` cache.
3. **Wire callers**: `telemetry.rs`, `update/config.rs`,
   `onboarding/detection.rs`, `setup_cmd.rs`.
4. **Auto-baseline** in `setup_cmd::run_setup` after `version_check`.
5. **Verify-only fallback** at the top of `setup_cmd::run_setup`.
6. **Banner** in `main.rs` after telemetry init, before dispatch.
7. **Tests**: unit (detector), integration (setup with sandbox
   forced on).
8. **Docs**: rewrite `docs/ai-sandboxes.md` to reflect seamless
   defaults; add a row to `docs/faq.md` for the override.

## See Also

- [PRD-010: CI Detection](010-ci-detection-integration.md) — the
  original CI heuristic this PRD generalizes
- [PRD-043: Drift Detection](043-configuration-drift-detection.md)
  — auto-baseline depends on the drift state-file format
- [PRD-022: Remote Telemetry](022-remote-telemetry-monitoring.md)
  — opt-in semantics this PRD preserves in seamless mode
- `docs/ai-sandboxes.md` — user-facing doc this PRD makes accurate
- `docs/operations/telemetry-forwarder.md` — multi-tenant concern
  this PRD addresses on the client side
