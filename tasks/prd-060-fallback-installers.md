# PRD-060 — Generalized Fallback Installers

- **Status:** proposed
- **Created:** 2026-08-03
- **Priority:** medium
- **Estimated:** 4 days
- **Depends on:** PRD-057 (tool freshness advisory — shipped; consumer of the install-receipt design decision below)

## Problem

Jarvy's platform rule is strict and correct: no first-party
winget/choco/scoop manifest → OMIT the Windows block, emit
`tool.unsupported`, done. That closes the namespace-squatting
supply-chain hole, but it leaves a long tail of tools that ARE
installable on the "unsupported" platform through a language-ecosystem
toolchain the user already has:

- `betterleaks` → `go install github.com/betterleaks/betterleaks@latest`
- `cfn-lint`, `locust`, `glances` → `pip install`
- `cypress` → `npm install -g` (npm is already its only route)
- `kafkactl`, `helmfile`, `kubent`, `velero`, `kubeseal`, `clusterctl`,
  `temporal`, `nats-server`, `argo`, `emqx`, `kcat`, `allure`,
  `structurizr`, `rabbitmq`, `nebula`, `stow`, `microk8s`, `composer` —
  ~20 tools carry a dated "No first-party winget manifest as of
  YYYY-MM" comment and a dead-end UX on Windows.

Today the only escape hatch is a bespoke `custom_install` fn per tool
(betterleaks just grew one, commit 4cadf92). That doesn't scale: each
one re-implements toolchain detection, version-pin mapping, and the
"toolchain missing" message, with no shared validation or telemetry.

## Goal

One declarative macro slot that says "if no native package manager
covers this OS, these first-party ecosystem packages install the same
binary" — with shared runtime, shared trust gates, shared telemetry,
and an install-receipt so `check-updates` can still track the tool.

## Non-Goals

- Fallbacks replacing native routes. Platform slots always win when
  present; fallback fires ONLY on `is_no_platform_installer()`.
- Auto-installing the ecosystem toolchain itself. If `go` isn't on
  PATH, we print the hint and stop — we do not bootstrap Go to install
  betterleaks.
- Arbitrary shell fallbacks (`curl | sh`). Ecosystem package managers
  only — they give us naming authority, checksums, and uninstall.
- Direct-binary-download fallbacks. That's the
  `pinned_binary_installer` domain (sha256-pinned, bash-only) and
  stays separate.

## Design

### 1. Macro slot

```rust
define_tool!(BETTERLEAKS, {
    command: "betterleaks",
    macos: { brew: "betterleaks" },
    linux: { brew: "betterleaks" },
    fallback: {
        go: "github.com/betterleaks/betterleaks",
    },
    // ...
});
```

Supported ecosystems in v1: `go`, `npm`, `cargo`, `pip`. Declared
order = attempt order. The slot compiles into a
`&'static [FallbackRoute]` on `ToolSpec`:

```rust
pub enum FallbackEco { Go, Npm, Cargo, Pip }
pub struct FallbackRoute { pub eco: FallbackEco, pub package: &'static str }
```

### 2. Runtime

In the install path, after platform-slot resolution:

1. Platform slot exists for current OS → use it, fallbacks never
   considered.
2. `is_no_platform_installer()` → iterate `fallback` routes in
   declared order; first route whose toolchain binary (`go`, `npm`,
   `cargo`, `pip`/`pip3`) is on PATH wins.
3. No route usable (no fallbacks declared, or no toolchain present) →
   existing `tool.unsupported` UX, now with a `fallback_hint` line
   naming the toolchains that would have worked ("install go, then
   re-run").

Version-pin mapping per ecosystem (concrete pins only — ranges/`latest`
fall back to the ecosystem's latest):

| Eco | Spec |
|---|---|
| go | `<module>@v<pin>` / `<module>@latest` (charset guard: digits+dots, ≤3 segments — generalize `go_module_spec` from betterleaks) |
| npm | `<pkg>@<pin>` / `<pkg>@latest` via `npm install -g` |
| cargo | `cargo install <crate> --version <pin>` / bare |
| pip | `pip install <pkg>==<pin>` / bare |

### 3. Trust gates

- Fallback IDs are compile-time constants in tool definitions —
  first-party upstream packages only, same review bar as winget IDs.
- Defense-in-depth: package strings run through the existing
  `validate_package_name` gauntlet at install time (leading-`-`, URL
  schemes, shell-meta, control bytes), and version pins through
  `validate_package_version`. A future registry-synced tool definition
  carrying a hostile fallback string hits the same wall as `[npm]`
  entries.
- Remote configs: no new gate needed — fallbacks are properties of the
  tool definition, not the config. A remote config can only name
  tools; the routes are ours.

### 4. Install receipt (the check-updates tension)

Fallback installs bypass the native package manager, so PRD-057's
backend router would misattribute them (e.g. ask brew about a
go-installed binary) or dump them in `unchecked`. Fix: record the
route.

- `~/.jarvy/install-receipts.json` — same pattern as
  `maintenance/cache.rs` / `doctor_cache.rs` (schema version, atomic
  0600 write, corrupt → empty):
  `{ "<command>": { "route": "fallback_go", "package": "<id>", "version": "<resolved|latest>", "installed_at_unix": ... } }`
- `check-updates` consults receipts BEFORE the OS backend router: a
  `fallback_go` receipt routes the tool to the `go list -m -versions`
  probe (already implemented for `[go]` packages), `fallback_npm` to
  `npm view`, etc.
- No receipt → current behavior unchanged. Receipts written only by
  the fallback path; native installs keep relying on the OS router.

### 5. First consumer + migration

- betterleaks: delete the bespoke `custom_install` + `go_module_spec`
  (fold the charset guard into the shared go route), declare
  `fallback: { go: "github.com/betterleaks/betterleaks" }`. Issue #78
  (winget tracking) stays open — a first-party winget manifest still
  beats the fallback.
- Candidates (audit each upstream for a first-party ecosystem package
  before migrating — verification rule applies): cfn-lint (pip),
  locust (pip), glances (pip), kafkactl (go), helmfile (go), kubent
  (go), velero (go), kubeseal (go), clusterctl (go), temporal (go),
  nats-server (go), argo (go), composer (n/a — investigate), allure
  (npm?), structurizr/emqx/kcat/rabbitmq/nebula/stow/microk8s (likely
  no ecosystem route — keep unsupported).
- Migrated tools update their dated comment to name the fallback route
  instead of a dead end.

## Telemetry

| Event | Fields | Notes |
|---|---|---|
| `tool.installed` | existing + `install_route = "platform" \| "fallback_go" \| "fallback_npm" \| "fallback_cargo" \| "fallback_pip"` | extend existing event; bounded |
| `tool.unsupported` | existing + `fallback_declared` (bool), `fallback_blocked = "no_toolchain" \| "none_declared"` | fires only when fallbacks also unusable |
| `tool.fallback_failed` | `tool`, `eco`, `error_kind` | new, warn — fallback attempted, subprocess failed |
| `maintenance.receipt_routed` | `tool`, `route` | new, debug — check-updates used a receipt instead of the OS router |

All gated by `telemetry_gate::is_enabled()` per the standing contract.

## Acceptance Criteria

1. On Windows with go on PATH, `betterleaks = "latest"` installs via
   `go install` with zero tool-specific code; with a `1.7.3` pin, via
   `@v1.7.3`.
2. On Windows with no ecosystem toolchain, the same config emits
   `tool.unsupported` with the toolchain hint; exit-code semantics
   unchanged (8 only when ALL tools unknown).
3. Platform slots always shadow fallbacks — a macOS box with both brew
   and go never touches the go route.
4. A fallback-installed tool appears in `jarvy check-updates` with the
   correct latest-version comparison (receipt-routed), not in
   `unchecked`.
5. Hostile fallback strings (leading `-`, shell-meta) are refused by
   `validate_package_name` with a test proving it.
6. betterleaks's bespoke `custom_install` is deleted; its three
   existing unit tests port to the shared route.

## Phasing

- **Phase 1 (~2.5d):** `FallbackRoute` type + macro slot + runtime
  (go/npm/cargo/pip), validation gauntlet wiring, telemetry, betterleaks
  migration, unit + integration tests.
- **Phase 2 (~1.5d):** install receipts + check-updates routing,
  candidate-tool migration audit (verify first-party packages, migrate
  the ones that check out), doc updates.

## Open Questions

1. Should `jarvy upgrade` also consult receipts (re-run the fallback
   route) or stay native-only in v1?
2. `pip` vs `pipx` for CLI tools — pipx isolates but is less commonly
   present. v1 proposal: `pip` (matches the `[pip]` package handler);
   revisit if PEP-668 externally-managed-environment errors bite.
3. Receipt staleness: user manually reinstalls via brew after a
   fallback install — receipt now lies. Mitigation: receipt validity
   could reuse the doctor-cache stat-signature trick (path+mtime+size).
   Decide in Phase 2.
