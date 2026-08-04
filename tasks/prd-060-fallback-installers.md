# PRD-060 — Generalized Fallback Installers

- **Status:** in_progress (Phase 1 complete 2026-08-03; Phase 2 pending)
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
- **HARD REQUIREMENT — no arbitrary shell fallbacks (`curl | sh`).**
  Ecosystem package managers only — they give us naming authority,
  checksums, and uninstall. This is a security invariant, not a
  scoping choice; any future extension of the fallback set must be an
  ecosystem package manager with a naming registry.
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

Supported ecosystems in v1: `go`, `npm`, `cargo`, `uv` (Python —
`uv tool install`, NOT `pip`; see §2a). Declared order = attempt
order. The slot compiles into a `&'static [FallbackRoute]` on
`ToolSpec`:

```rust
pub enum FallbackEco { Go, Npm, Cargo, Uv }
pub struct FallbackRoute { pub eco: FallbackEco, pub package: &'static str }
```

### 2. Runtime

In the install path, after platform-slot resolution:

1. Platform slot exists for current OS → use it, fallbacks never
   considered.
2. `is_no_platform_installer()` → take the FIRST declared fallback
   route. If its toolchain binary is on PATH, run it.
3. Toolchain missing → **install the toolchain through jarvy's own
   registry, then run the fallback.** The user listed the tool in
   their config — that's the same consent that lets `depends_on`
   install missing deps today ("strict missing = warn + install
   anyway"). Every v1 toolchain is already a registered jarvy tool
   with first-party routes on all platforms: `go` (winget
   `GoLang.Go`), node via `nvm`/`node` (existing flexible-dep
   pattern), `rustup` for cargo, `uv` (winget `astral-sh.uv`).
   Mechanically this reuses the `depends_on_one_of` machinery — a
   fallback route implies a toolchain dependency, resolved through
   the normal install path (npm maps to `["node", "nvm"]`).
4. Toolchain itself uninstallable on this platform → try the next
   declared route. All routes exhausted → existing `tool.unsupported`
   UX with a `fallback_hint` naming what would have worked.

### 2a. Python route = uv, not pip

`uv tool install` is the Python route: isolated per-tool environments
(pipx semantics), no venv/PEP-668 externally-managed-environment
errors, no site-packages pollution, and uv itself is a single static
binary with a first-party winget manifest — the cleanest toolchain to
auto-install in step 3. `pip` is deliberately NOT a fallback eco (the
`[pip]` package section is unaffected).

Version-pin mapping per ecosystem (concrete pins only — ranges/`latest`
fall back to the ecosystem's latest):

| Eco | Spec |
|---|---|
| go | `<module>@v<pin>` / `<module>@latest` (charset guard: digits+dots, ≤3 segments — generalize `go_module_spec` from betterleaks) |
| npm | `<pkg>@<pin>` / `<pkg>@latest` via `npm install -g` |
| cargo | `cargo install <crate> --version <pin>` / bare |
| uv | `uv tool install <pkg>==<pin>` / bare |

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

### 4. Install receipts (check-updates + upgrade attribution)

Fallback installs bypass the native package manager, so PRD-057's
backend router would misattribute them or dump them in `unchecked`.
Fix: record the route, with staleness protection, wired into each
existing consumer deliberately.

**Store.** `~/.jarvy/install-receipts.json` — same pattern as
`maintenance/cache.rs` / `doctor_cache.rs` (schema version, atomic
tmp-write + rename + 0600, corrupt file → empty usable store,
`JARVY_TEST_HOME` honored):

```json
{ "<command>": {
    "route": "fallback_go",
    "package": "github.com/betterleaks/betterleaks",
    "version": "1.7.3",
    "installed_at_unix": 1754265600,
    "bin_path": "...", "bin_mtime_unix": 0, "bin_size": 0 } }
```

The `bin_*` stat signature reuses the doctor-cache trick
(path + mtime + size, captured right after the fallback install
succeeds). A receipt is **valid** only while the live binary's stat
still matches. User later reinstalls via brew/winget → stat diverges
→ receipt treated as absent and pruned. This closes the
stale-receipt-lies problem by construction instead of deferring it.

**Resolver integration** (`maintenance/resolver.rs`). In the
provisioner loop, after the `ignored` / `version_manager` gates and
before the spec router: valid receipt → build a `CheckTarget` with
the receipt's ecosystem backend + `pkg_id` = receipt package
(`fallback_go` → existing `GoBackend`, `fallback_npm` → `NpmBackend`,
`fallback_cargo` → `CargoBackend`, `fallback_uv` → new `UvBackend`).
Invalid/absent receipt → existing chain unchanged (`custom_install` →
`provisioner_backend` → `no_backend_for_platform`). Compatibility
points, checked against the current implementation:

- **`installed` probe**: unchanged — `detect_installed_version
  (spec.command)` is route-agnostic (binary `--version` probe).
- **`ResolveMode::Cheap`** (the `jarvy setup` summary path, perf F1):
  receipt lookup is a sub-millisecond disk read, no subprocess — the
  cheap path stays cheap. The receipts file is loaded once per
  resolve, not per tool.
- **Cache** (`maintenance/cache.rs`, 24h TTL): cache entries gain a
  `route` field; an entry whose route differs from the current
  resolution is treated as a miss, so a route change (fallback →
  native reinstall) can't serve a wrong-backend `latest` for up to
  24h.
- **Dedup**: `seen` is keyed by config name, so provisioner
  `betterleaks` and an explicit `[go]`
  `github.com/betterleaks/betterleaks` don't collide there —
  the resolver additionally skips a receipt-routed target whose
  `pkg_id` already appeared, first declaration wins.
- **`push_if_safe` gauntlet**: receipt `package` strings pass through
  the same gate as `[go]`/`[npm]` keys before reaching backend argv —
  the receipts file is user-writable state, treat it like hostile
  TOML.
- **UvBackend**: installed side from `uv tool list` (one subprocess,
  batched like `detect_npm_globals`); latest side from PyPI — probe
  command decided in Phase 2 (candidates: `uv tool upgrade --dry-run`
  parse, or PyPI JSON API via the existing bounded-fetch HTTPS
  helper).

**Route-agnostic consumers — verified no changes needed**: `drift`
(baseline = binary version probes), `lock` (same), `export`
(introspects installed binaries), `doctor`/doctor-cache (stat-keyed
binary probes), skip-detection (`cmd_satisfies`). All operate on the
binary, not the install channel.

**`jarvy upgrade`** (was Open Question 1 — now resolved: yes,
Phase 2): with a valid receipt, upgrade re-runs the fallback route at
`latest`/new pin and refreshes the receipt's stat signature.
Without receipt-awareness, upgrade on the fallback platform would hit
the no-platform-installer dead end — the receipt is what makes
upgrade work at all there.

**Precedence when a native route later appears** (e.g. issue #78
lands a betterleaks winget manifest and the tool def gains a
`windows:` block): valid receipt still wins for check-updates
attribution — the go-installed binary is what's on PATH, and asking
winget about it produces garbage. The user migrates by reinstalling
natively (stat diverges → receipt pruned → native router takes over)
or via `jarvy upgrade --native` (Phase 2 flag, explicitly re-installs
through the platform slot and deletes the receipt).

**Writers**: fallback install path and receipt-aware upgrade only.
Single-writer-per-invocation + atomic rename; the background
check-updates refresher only reads.

### 5. First consumer + migration

- betterleaks: delete the bespoke `custom_install` + `go_module_spec`
  (fold the charset guard into the shared go route), declare
  `fallback: { go: "github.com/betterleaks/betterleaks" }`. Issue #78
  (winget tracking) stays open — a first-party winget manifest still
  beats the fallback.
- Candidates (audit each upstream for a first-party ecosystem package
  before migrating — verification rule applies): cfn-lint (uv),
  locust (uv), glances (uv), kafkactl (go), helmfile (go), kubent
  (go), velero (go), kubeseal (go), clusterctl (go), temporal (go),
  nats-server (go), argo (go), composer (n/a — investigate), allure
  (npm?), structurizr/emqx/kcat/rabbitmq/nebula/stow/microk8s (likely
  no ecosystem route — keep unsupported).
- Migrated tools update their dated comment to name the fallback route
  instead of a dead end.

## Telemetry

| Event | Fields | Notes |
|---|---|---|
| `tool.installed` | existing + `install_route = "platform" \| "fallback_go" \| "fallback_npm" \| "fallback_cargo" \| "fallback_uv"`, `toolchain_bootstrapped` (bool — step 3 installed the toolchain first) | extend existing event; bounded |
| `tool.unsupported` | existing + `fallback_declared` (bool), `fallback_blocked = "toolchain_uninstallable" \| "none_declared"` | fires only when all fallback routes exhausted |
| `tool.fallback_failed` | `tool`, `eco`, `error_kind` | new, warn — fallback attempted, subprocess failed |
| `maintenance.receipt_routed` | `tool`, `route` | new, debug — check-updates used a receipt instead of the OS router |
| `maintenance.receipt_stale` | `tool`, `route` | new, debug — stat signature diverged; receipt pruned, native router took over |

All gated by `telemetry_gate::is_enabled()` per the standing contract.

## Acceptance Criteria

1. On Windows with go on PATH, `betterleaks = "latest"` installs via
   `go install` with zero tool-specific code; with a `1.7.3` pin, via
   `@v1.7.3`.
2. On Windows with no ecosystem toolchain, the same config installs
   go through jarvy's registry (winget `GoLang.Go`) and then runs the
   fallback; `tool.installed` carries `toolchain_bootstrapped = true`.
   `tool.unsupported` (with fallback hint, exit-code semantics
   unchanged — 8 only when ALL tools unknown) fires only when the
   toolchain itself is uninstallable.
3. Platform slots always shadow fallbacks — a macOS box with both brew
   and go never touches the go route.
4. A fallback-installed tool appears in `jarvy check-updates` with the
   correct latest-version comparison (receipt-routed), not in
   `unchecked`.
5. Hostile fallback strings (leading `-`, shell-meta) are refused by
   `validate_package_name` with a test proving it.
6. betterleaks's bespoke `custom_install` is deleted; its three
   existing unit tests port to the shared route.
7. A receipt whose stat signature no longer matches the live binary is
   pruned and the tool routes through the native backend (test:
   fallback install → overwrite binary → check-updates attributes
   natively, `maintenance.receipt_stale` emitted).
8. `jarvy upgrade` on a receipt-holding tool re-runs the fallback
   route and refreshes the receipt.

## Phasing

- **Phase 1 (~2.5d):** `FallbackRoute` type + macro slot + runtime
  (go/npm/cargo/uv incl. toolchain bootstrap via registry), validation
  gauntlet wiring, receipt WRITES (cheap — the store module is a
  doctor-cache clone), telemetry, betterleaks migration, unit +
  integration tests.
- **Phase 2 (~1.5d):** receipt READS — check-updates resolver routing
  + `UvBackend` + cache `route` field, receipt-aware `jarvy upgrade`
  (+ `--native` migration flag), candidate-tool migration audit
  (verify first-party packages per the verification rule), doc
  updates.

Receipt writes land in Phase 1 so Phase-1 fallback installs are
already attributable when Phase 2 ships — no receipt backfill needed.

## Open Questions — DECIDED (Phase 1, 2026-08-03)

1. UvBackend latest-version probe: **PyPI JSON API via the existing
   `net::bounded_fetch` helper.** Sturdier than parsing
   `uv tool upgrade --dry-run` output (uv output format is not a
   stable contract); bounded_fetch already carries the HTTPS-only /
   size-cap discipline. Implement in Phase 2.
2. Toolchain bootstrap on distros with ancient native toolchains:
   **tool-def ordering as-is.** Each toolchain's own tool def already
   encodes the distro-vs-linuxbrew preference; the bootstrap path
   (`registry::add(toolchain, "latest")`) inherits it. No special
   casing.
3. `jarvy diff` / dry-run preview of the fallback route: **yes** —
   show "would install via go install …". Phase 2 alongside receipt
   READS.

Also settled during Phase 1: **already-installed tools never enter
fallback.** `ensure()`'s `cmd_satisfies` skip-detection fires before
`install()`, so pre-existing installs get no receipt and keep their
current attribution (`install_route` fields only describe installs
jarvy itself performed this process).
