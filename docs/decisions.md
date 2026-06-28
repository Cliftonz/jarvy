---
title: "Architecture decisions — Jarvy"
description: "Index of architectural decisions and their rationale: PRDs, design choices, and trust boundaries that shape Jarvy."
tags:
  - architecture
  - contributing
---

# Architecture decisions

Jarvy's design choices are captured in three places. This page is the index — use it as the entry point when you want to know *why* something is the way it is.

| Where | What lives there |
|-------|------------------|
| `prd/*.md` (in repo) | PRDs — the long-form rationale for each major feature, including alternatives considered and rejected |
| `CLAUDE.md` (in repo root) | Non-obvious invariants, trust boundaries, and conventions that can't be derived from reading the code |
| This page | A curated index of the highest-leverage decisions with one-line summaries and links |

PRDs are kept indefinitely. They are the system's memory: "why did we decide that `cargo install` would not honor `--features` from `[cargo]`?" — answered by the PRD that introduced the section, even if the field has since been refactored.

---

## Trust & security

| Decision | Rationale | Reference |
|----------|-----------|-----------|
| Remote configs (`jarvy setup --from <url>`) may **narrow** trust but never **broaden** it | Defends against a friendly-looking remote `jarvy.toml` that enables `allow_custom_commands` or `allow_remote_packages` on a victim's machine | `CLAUDE.md` § Trust Boundaries; `src/ai_hooks/`, `src/packages/mod.rs` |
| Telemetry is **opt-out by default, auto-disabled in CI / sandboxes** | Predictable: dashboards don't fill with phantom CI runs; users don't ship usage data they didn't agree to | `prd/022-remote-telemetry-monitoring.md`; `src/telemetry.rs` |
| MCP mutating tools require interactive TTY confirm + rate limit + audit log | LLM-driven workflows must not silently mutate a workspace; the human stays in the loop for destructive ops | `prd/021-mcp-server.md`; `src/mcp/extended_tools.rs::gate_mutation` |
| Package names / versions rejected if they look like CLI flags or URL schemes | `npm install --registry=evil` style attacks via a hostile `jarvy.toml` | `src/packages/common.rs::validate_package_name` |
| Self-update verifies cosign signatures on the binary | Compromised mirror can't ship a malicious replacement | `prd/035-self-updating.md`; `src/update/signature.rs` |
| ANSI / control bytes refused in package names + version strings | Hostile `jarvy.toml` could plant ANSI sequences in the dry-run preview that operators rely on as "safe to inspect" | `src/packages/common.rs::validate_package_name` (control-byte branch) |

---

## Architecture

| Decision | Rationale | Reference |
|----------|-----------|-----------|
| One `define_tool!` macro invocation per tool, registered via `tools::register_all()` | New tools land in 15 lines; macro slots enforce cross-platform package mapping at compile time | `prd/002-tool-spec-abstraction.md`; `src/tools/spec.rs` |
| Top-level sections pinned by `TOP_LEVEL_SECTIONS` const + destructure test | Adding a `Config` field without updating the const fails compilation, not validation later | `src/config.rs::TOP_LEVEL_SECTIONS`; `tests::top_level_sections_matches_config_fields` |
| Roles support inheritance + override | Real-world teams have base + per-role specialization (junior-backend extends backend extends developer) | `prd/033-role-based-configurations.md`; `src/roles/` |
| Drift state baseline lives in `.jarvy/state.json`, not git | Each developer's machine state is local; comparing across teammates is intentionally not a feature | `prd/043-configuration-drift-detection.md`; `src/drift/` |
| Network config priority: env > tool > global | Env wins so debugging proxy issues doesn't require editing config; tool wins next so one-off overrides work | `src/network/`; `CLAUDE.md` § Module Map |
| `main.rs` is process init only; dispatch + handlers live in `commands::dispatch` | Keeps the binary entry point reviewable; per-command handlers stay isolated and testable | `prd/037-main-rs-code-maintainability.md`; `src/commands/dispatch.rs` |

---

## Conventions

| Decision | Rationale |
|----------|-----------|
| **Conventional Commits** for every commit | Generates the changelog without manual curation; reviewers can scan history at a glance |
| **Rust 2024 edition** | Modern lifetime elision, latest formatter defaults |
| **`cargo fmt` + `cargo clippy -D warnings`** as CI gates | Style debates resolved by the tool, not the reviewer |
| **No new dependencies without justification** | Each crate is supply-chain attack surface; prefer stdlib + existing deps |
| **Logs in `~/.jarvy/logs/`, rotated daily** | Support tickets need a consistent place to look; rotation prevents unbounded growth |
| **Tickets are ZIP bundles at `~/.jarvy/tickets/`** | One-file handoff to support; PII redacted by default; user runs `jarvy ticket show` to inspect before sending |

---

## How to propose a new decision

1. Open a PRD draft in `prd/NNN-short-name.md` describing problem, evidence, alternatives, and recommendation
2. Tag the relevant module owners in the PR
3. Reach alignment in PR review — the merged PRD is the decision record
4. If the decision changes a trust boundary or invariant, update `CLAUDE.md` so future contributors don't have to re-derive it

This page is updated by reference, not by hand — when a PRD ships and the implementation lands, add a row here pointing at it.

---

## Related

- [Architecture overview](architecture.md) — module map, data flow
- [Contributing guide](contributing.md) — workflow, commit style
- [Contributing: testing](contributing-testing.md) — test layers, when to add what
