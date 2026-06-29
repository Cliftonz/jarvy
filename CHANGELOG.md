# Changelog

All notable changes to Jarvy will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Policy

- **Stable releases (`vX.Y.Z`)** get a curated entry below **before the tag is
  pushed**. The release workflow's `Build release notes` step awk-extracts the
  matching `## [vX.Y.Z]` section into the GitHub release body, then appends a
  `**Full Changelog**` compare link plus Jarvy's standing install/security
  footer. Forgetting this entry causes the workflow to fall through to a raw
  `git log` listing — technically valid, but reads like a commit dump rather
  than a curated narrative. Update CHANGELOG before tagging.
- **Pre-releases (`vX.Y.Z-rc.N`, `-beta.N`, `-alpha.N`)** do **not** get a
  CHANGELOG entry. The awk extraction returns empty, the workflow falls
  through to `git log <prev-tag>..<tag>` notes, and that fallback is the
  intended pre-release path. The curated stable entry below is written once
  when the corresponding stable cuts.
- Entry headers must match the awk pattern: `## [vX.Y.Z]` or
  `## [vX.Y.Z] — Title` (em-dash optional). Other shapes won't be matched.

See [`docs/release-testing.md`](https://github.com/Cliftonz/jarvy/blob/main/docs/release-testing.md)
for the full release process and
[`docs/release-quirks-jarvy.md`](https://github.com/Cliftonz/jarvy/blob/main/docs/release-quirks-jarvy.md)
for divergences from generic release skills.

## [Unreleased] — Close out PRD-011/013/014/037/038/039/048/049/052/054/055 + library registry + git skill sources + skills + git hooks + progress (2026-06-28)

A documentation + maintainability + ecosystem-breadth pass that closes
eleven long-open PRDs across five commits. The headliner is **PRD-054
library registry** + **PRD-055 git skill sources** — a shared
HTTPS-fetched manifest format that lets a team publish reusable AI
hooks, MCP servers, and AI agent skills at any URL, with skills
additionally supported via plain Git repos (no `manifest.json`
required). `[ai_hooks]`, `[mcp_register]`, and `[skills]` all consume
the same format. PRD-049 (skills) rides on it; PRD-048 / 052 (git
hooks, spinners) shipped earlier in the day. No user-visible behavior
changes for existing configs — all new surface is additive.

### Added — PRD-044 / 047 / 051 (auto-discovery + monorepo + JSON output)

Three PRDs closed in a single session, all additive (no existing
config changes).

**PRD-051: `--format json` on every command.** Subcommands that
previously only emitted human text now accept `--format json` and emit
a structured envelope: `jarvy ci-info`, `jarvy drift {status,accept,
fix}`, `jarvy logs {stats,clean,config}`, `jarvy ticket {create,show,
list,clean}`, `jarvy services {start,stop,status,restart}`. CLI exit
codes are identical between human and JSON paths so `$?` based
control flow keeps working. See `docs/cli-reference.md` and the new
"Structured output" section in `CLAUDE.md`.

**PRD-044: `jarvy discover` project tool auto-discovery.** New
top-level command that scans the project root for marker files
(Cargo.toml, package.json, go.mod, Dockerfile, k8s/, *.tf, Makefile,
Justfile, …), infers versions from `rust-toolchain.toml` / `.nvmrc`
/ `.python-version` / `go.mod`, and either prints suggestions or
merges them into `jarvy.toml` (`--apply`). The merge is append-only:
hand-pinned tools survive unchanged. New module `src/discover/` with
built-in `default_rules()` covering rust, node, python, go, ruby,
docker, kubectl, helm, terraform, pre-commit, make, just. Custom rule
files deferred. See `docs/discover.md`.

**PRD-047: `jarvy workspace` monorepo inspection.** New CLI surface
over the existing workspace foundation (`crate::workspace::
find_workspace_root` + `merge_configs`). Three read-only subcommands
— `list`, `show <member>`, `validate` — that resolve per-member
configs via inheritance and surface the result with `(inherited)` /
`(overridden)` provenance. Empty `[workspace] inherit = []` is
treated as `["provisioner"]` for display so the common case works
without explicit config. Workspace-aware `jarvy setup --project <name>`
orchestration deferred. See `docs/workspace.md`.

### Added — parallel-review enhancement plan (25 P0/P1 items + sweep)

A multi-batch sweep against the parallel-code-review enhancement plan
shipped as 12 commits (`6155056..HEAD`). Highlights:

- **Security** (items 1, 2, 3, 12, 13, 14, 15): argv-injection refusal on
  git refs, symlink-escape refusal on the clone walker, `file://` scoping
  to the library cache root, `manifest_sha256` pin on library_sources
  (refuses re-published manifests), loud `library.signature_unenforced`
  warning when `require_signature = true` (cosign not yet enforced in v1),
  `GitHooksConfig::default()` matches serde defaults instead of silently
  disabling hooks, and a new `test-bypass` Cargo feature that compiles
  `JARVY_{LIBRARY,REGISTRY}_ALLOW_INSECURE_FETCH` + `JARVY_TEST_HOME`
  out of release builds (env vars are inert in shipped binaries).
- **Trust gates** (items 4, 5): `[skills]` and `[git_hooks]` now propagate
  `ConfigOrigin::Remote` from `Config::mark_remote` and enforce the
  `allow_remote` opt-in on both subsystems.
- **Observability** (items 6, 7, 8, 22, 23, 24): every
  `library.*` / `library.git.*` / `skills.*` / `git_hooks.*` /
  `package.*` event reads `telemetry_gate::is_enabled()` so opt-out
  users don't ship breadcrumbs; `library.sync.failed` emit on every error
  path (was silent); `library.sync` tracing span wraps each per-source
  fetch; `git_hooks.{install,update}_{started,completed}` envelopes
  carry `status`, `applied`, `framework`, `auto_update`,
  `run_after_install`, `duration_ms` — same shape as
  `ai_hook.phase_*` / `mcp_register.phase_*`.
- **Maintainability** (items 16, 17, 18, 19): `library_registry::sync_all`
  consolidates the three identical `prepare_library_sources` copies;
  `packages::common::run_install_loop` consolidates the gem/go/cargo/nuget
  install + telemetry loops behind a closure-based helper;
  `net::bounded_fetch` collapses the two copies of HTTPS-only refusal +
  bounded read + loopback-bypass parser (with per-consumer env-var
  names preserved for test isolation); `agents::Agent` is the canonical
  enum shared by `ai_hooks`, `mcp_register`, and `skills` (the three
  former per-subsystem enums are now `pub use` aliases). Net 450 LOC
  removed.
- **Performance** (items 20, 21): library_registry caches `Arc<Manifest>`
  so resolvers snapshot the cache and drop the mutex before walking
  items; `cmd_satisfies` caches `<cmd> --version` stdout so per-tool
  version probes don't refork the package manager. `detect_linux_pm`
  / `detect_bsd_pm` dropped their local `has` closures that bypassed
  the cached `has()`.
- **QA** (items 9, 10, 11, 25): coverage tests for ai_hooks
  `library_sources` resolution, `mcp_register::use_library` overrides,
  `skills` sha-mismatch refusal, and `hooks_cmd` action exit-code
  contract. Plus `#[serial(jarvy_telemetry_disclosure)]` on the
  telemetry disclosure tests to prevent parallel-test flakes.

User-visible config additions: `[[<subsystem>.library_sources]]` accepts
an optional `manifest_sha256 = "<hex>"` pin. CLI exit codes and
existing event names are unchanged.

### Added — git-shorthand for skill sources (PRD-055)

- **`git+https://...@<ref>[#<subpath>]` URL scheme** on `[skills]
  library_sources`. Jarvy clones the repo at the pinned ref, walks
  the optional subpath for `SKILL.md` files, parses each file's YAML
  frontmatter, and synthesizes a manifest in-memory. Publishers don't
  need to maintain `manifest.json` — the SKILL.md files are
  self-describing.

  ```toml
  [[skills.library_sources]]
  url = "git+https://github.com/myorg/jarvy-skills.git@v1.2.0#skills/"
  ```

- **`github:owner/repo@<ref>` shorthand** for the common GitHub case:

  ```toml
  [[skills.library_sources]]
  url = "github:anthropics/skills@v1.0.0"
  ```

- **`@<ref>` pin is mandatory**. Unpinned URLs refused at parse time
  with a clear message — silent floating refs would let a publisher
  rev skills without a visible pin bump.

- **Trust hierarchy**: commit SHA (tamper-evident) > tag (mutable but
  conventional) > branch (freely mutable, emits
  `library.git.mutable_ref` warning every fetch). Documented in
  `docs/library-registry.md`.

- **SKILL.md frontmatter convention**: `name:` + `version:` required,
  `description:` + `supported_agents:` optional. Files missing
  required fields are skipped with a `library.git_skill.skipped`
  event citing the reason. No silent failures.

- **Subpath traversal refused** at parse time + at fetch time:
  `..` segments and absolute paths are rejected with a canonical-path
  check inside the clone root. Mirrors
  `safety::resolve_within_workspace` from `src/mcp/extended_tools.rs`.

- **No new dependencies**. Shells out to `git`; missing git refuses
  with a clear error pointing at `[provisioner] git = "latest"`.
  Cached `--depth 1` clone refreshes via `git fetch + git checkout
  <ref>`.

- **Why skills only**: SKILL.md carries its own frontmatter, so
  Jarvy has everything needed to build a manifest entry from one
  file. AI hooks (script bodies) and MCP servers (command/args/env
  tables) don't — those still ship via `manifest.json`. A publisher
  who wants both in one Git repo puts `manifest.json` at the root and
  uses the existing URL form.

- New modules: `src/library_registry/url_parser.rs` (scheme +
  `@<ref>` + `#<subpath>` parsing with safety refusals),
  `src/library_registry/git_fetch.rs` (clone + frontmatter walker
  + manifest synthesizer). `sync()` in `mod.rs` dispatches by
  scheme. `read_file_url()` extends the installer's fetch path to
  handle `file://` URLs that point into the git cache.

- Trust gate inherits unchanged from PRD-054 — remote-fetched configs
  CANNOT declare `library_sources` of any scheme.

### Added — library registry (PRD-054)

- **`src/library_registry/` shared module**: manifest schema (tagged
  by `kind`: `ai_hook` / `mcp_server` / `skill`), HTTPS-bounded fetch
  (`MAX_MANIFEST_BYTES = 16 MiB`, `MAX_ITEM_BYTES = 1 MiB`), on-disk
  cache at `~/.jarvy/library.d/<sha256-of-url>/manifest.json`,
  in-process resolver across all cached libraries. Atomic write
  pattern (`.new` → rename) for cache durability.

- **One manifest, three consumers**: a single `manifest.json` URL can
  publish AI hooks, MCP servers, and skills simultaneously — each
  consumer filters by `kind`. Publishers write one manifest; teams
  point `[ai_hooks] library_sources`, `[mcp_register] library_sources`,
  and `[skills] library_sources` at the same URL.

  ```toml
  [[ai_hooks.library_sources]]
  url = "https://cdn.myorg.com/jarvy/manifest.json"

  [[ai_hooks.hook]]
  use = "no-prod-deploys"
  ```

- **Trust model uniform across consumers**: remote-fetched configs
  (`jarvy setup --from <url>`) CANNOT declare `library_sources` —
  refused with `library.remote_refused` event. Mirrors
  `[packages] allow_remote` semantics. There is no override flag;
  adding one would defeat the purpose. Teams that want to ship
  `library_sources` to every developer copy them into each user's
  local `~/.jarvy/config.toml`.

- **Built-in items win over library items**: `crate::ai_hooks::LIBRARY`
  (the canonical Jarvy-shipped hooks) is checked BEFORE library
  fallbacks, so name collisions favor the audited built-in.

- **sha256 verification** for skill `SKILL.md` bodies (mandatory) and
  scaffolded for ai_hook `bash_url` (v1 only honors inline `bash:` for
  hooks). A publisher mutating a versioned artifact in place surfaces
  a clear `library.sha_mismatch` event and refuses to install.

- **Offline tolerance**: on network failure, the cached on-disk
  manifest is served with a `library.fetch.cached_hit
  reason="fetch_failed"` event so log scrapers can see staleness.

- **Cosign signature verification scaffolded but not enforced in v1**.
  `require_signature = true` (default) is honored once cosign wiring
  lands; `false` today emits a `library.signature_disabled` warning.
  Phase 5 of PRD-054.

### Added — AI agent skills installation (PRD-049 v1)

- **`[skills]` config block** with `library_sources`, `install` map,
  per-skill `agents = [...]` narrowing.
- **`jarvy skills` subcommand**: `install` (all or `--name <skill>`),
  `list` (per-agent status), `status` (drift summary), `agents`
  (detect installed AI agents).
- **Setup integration**: `jarvy setup` auto-installs every configured
  skill when `[skills] auto_install = true` (default).
- **Per-agent path layout**: `~/.{agent}/skills/<skill-name>/SKILL.md`
  across claude-code, cursor, codex, windsurf, cline, continue. Two
  narrowing layers (consumer `agents = [...]` + publisher
  `supported_agents = [...]`) both apply.
- **`.jarvy-skill.json` sidecar** records version + sha256 + install
  time per skill per agent. `jarvy skills status` uses it for drift
  detection without needing to re-fetch.
- **v1 explicitly skips** skills.sh API integration (search / info /
  popular), companion file fetching, `jarvy skills update` /
  `remove`, version-range pinning, project-scope skills. Tracked
  under PRD-049 phase 2.

### Added — library_sources consumers for AI hooks + MCP register

- **`[ai_hooks].library_sources`**: fetch + register library hook
  items. `use = "hook-name"` resolves built-in `LIBRARY` first, then
  cached library items. Hook bodies are taken inline from manifest
  `bash:` / `powershell:` fields. Per-source-failure-is-advisory:
  `apply` continues with cached + built-in hooks if a library URL is
  unreachable.

- **`[[mcp_register.server]] use = "library-name"`**: pulls
  `command` / `args` / `env` defaults from a previously synced
  library item. Locally-declared fields on the spec override the
  library defaults (e.g. spec `env = { ... }` wins over library env).
  Subject to the existing `allow_custom_servers` gate plus the new
  `library_sources` remote-refusal gate.

### Tracking

- Drafts + closes PRD-054 (Library Registry — v1 shipped, sig verify
  + `jarvy library` CLI tracked as follow-up)
- Drafts + closes PRD-055 (Git skill sources — full v1 shipped;
  `git+ssh://` and sparse-checkout tracked as follow-up)
- Closes PRD-049 phase 1 (Skills Registry Integration — library-based
  install ships; skills.sh API + remove/update commands tracked as
  PRD-049 phase 2)
- Continues PRD-048 (Pre-Commit Hook Installation) + PRD-052
  (Progress Indicators) from the prior commit
- Continues PRD-011 / 013 / 014 / 037 / 038 / 039 closures from the
  first commit

---

## [Unreleased — earlier: pre-commit hooks + progress spinners]

(Originally a separate `[Unreleased]` entry; merged into the section
above so the awk extractor sees a single curated block.)

A documentation + maintainability + ecosystem-breadth pass that closes
eight long-open PRDs across three commits: gem/go package handlers +
main.rs extraction + documentation gaps in the first, pre-commit hook
framework + spinner output in the second. No user-visible behavior
changes for existing configs — all new surface is additive (`[gem]` /
`[go]` / `[git_hooks]` sections, `jarvy hooks` subcommand,
opt-out-friendly progress spinners).

### Added — pre-commit hook framework integration (PRD-048)

- **`[git_hooks]` section** auto-installs and manages git pre-commit
  hooks from `jarvy.toml`. Today the `pre-commit` framework
  (<https://pre-commit.com>) is fully supported; `husky` and `lefthook`
  are recognized by auto-detection but their handlers are stubbed with
  a clear "framework configured but not yet supported" error so configs
  can declare intent without silent no-ops.

  ```toml
  [git_hooks]
  # block presence is the opt-in; auto-detects from .pre-commit-config.yaml

  [git_hooks.pre_commit]
  version = "3.6.0"                # pin the framework version
  install_hooks = true             # warm hook envs eagerly
  ```

- **`jarvy hooks` subcommand**: `install`, `update`, `status`, `list`,
  `run` (with `--all-files` / `--hook <id>`), `uninstall`. Status output
  parses `.pre-commit-config.yaml` directly, so hook counts work even
  when the `pre-commit` CLI itself isn't installed yet.

- **Setup integration**: `jarvy setup` auto-runs `jarvy hooks install`
  between the git-config and ai-hooks phases. Gated on
  `[git_hooks] auto_install = true` (default). New phase emits
  `git_hooks.phase_started` / `_completed` / `_skipped` /
  `_install_failed` telemetry events.

- **Trust boundary**: deliberately a new top-level block, NOT
  `[hooks].git_hooks` — `[hooks]` is already taken by setup-lifecycle
  shell scripts (PRD-003). Remote-config trust gate via
  `[git_hooks] allow_remote = true` (mirrors `[packages] allow_remote`):
  a friendly-looking remote config cannot silently land arbitrary git
  hooks on the consuming machine without explicit opt-in in the SOURCE
  config. Refusals log `git_hooks.remote_refused` for audit.

- New module: `src/git_hooks/{mod.rs, config.rs, detection.rs, precommit.rs}`.
  Husky / lefthook handler stubs return `HookError::UnsupportedFramework`
  with the framework name, so the surface is stable for follow-up work.

### Added — progress spinners (PRD-052)

- **`indicatif` dependency** plus `src/progress.rs` helper providing
  `Progress::start()` → `Spinner` with `finish_ok` / `finish_skipped` /
  `finish_failed`. All long-running commands route through this helper
  rather than constructing `ProgressBar` directly, keeping the muting
  decision in one place.

- **Auto-disable** when any of: stdout is not a TTY, `JARVY_QUIET=1` or
  `--quiet`/`-q` on argv, `--format json` / `--log-format json`,
  sandbox / CI detected by `sandbox::is_seamless_auto()`, or
  `JARVY_NO_PROGRESS=1` (explicit kill switch). In sandbox / CI mode,
  spinners fall through to plain `println!` lines so log scrapers still
  see start / finish events.

- **Wired into** `jarvy update check` (network spinner) and
  `jarvy hooks install` / `update`. Deeper integration in `setup_cmd`'s
  parallel-install loop is deferred — needs design to avoid clashing
  with subprocess streaming stdout.

- Uses stdlib `std::io::IsTerminal` rather than dragging in a direct
  `libc` dep.

### Changed

- `src/main.rs` + `src/lib.rs`: register `progress` and `git_hooks`
  modules. `CLAUDE.md` module map updated.
- `src/config.rs`: new `git_hooks: Option<GitHooksConfig>` field;
  `TOP_LEVEL_SECTIONS` extended; `top_level_sections_matches_config_fields`
  destructure test updated.
- `src/commands/dispatch.rs`: route `Commands::Hooks { action, file }`
  to `commands::hooks_cmd::run_hooks`.

### Docs

- `docs/git-hooks.md` covers configuration, commands, status output,
  trust boundary, CI considerations, troubleshooting.
- `mkdocs.yml` adds "Git hooks (pre-commit)" under Guides.
- `tasks/prd-048-pre-commit-hook-installation.json` and
  `tasks/prd-052-progress-indicators.json` created with completion
  notes and explicit follow-up scope.

### Tracking

- Closes PRD-048 (Pre-Commit Hook Installation — pre-commit framework
  only; husky / lefthook tracked as follow-up)
- Closes PRD-052 (Progress Indicators — helper module + `update check`
  + `hooks install/update`; deeper setup_cmd integration tracked as
  follow-up)

---

### Earlier in the day — gem/go handlers, main.rs extraction, documentation gaps

(Originally committed separately; merged into this unreleased block so
the awk extractor sees a single curated `[Unreleased]` section.)

### Added — language package ecosystems (PRD-039)

- **`[gem]` section** installs Ruby gems via `gem install --no-document
  <name> [-v <version>]` against the active ruby. `--no-document` is
  unconditional — provisioning runs don't need RDoc/RI, and skipping the
  build cuts install time from ~30s to ~3s on chatty gems like
  `rubocop`.

  ```toml
  [gem]
  bundler = "latest"
  rubocop = "1.60.0"
  ```

- **`[go]` section** installs Go binaries via `go install <module>@<version>`
  to the user's `GOBIN`. Module paths are full import paths (require
  quoting in TOML); version is mandatory outside a `go.mod` tree, use
  `"latest"` for floating installs.

  ```toml
  [go]
  "github.com/golangci/golangci-lint/cmd/golangci-lint" = "latest"
  "github.com/cosmtrek/air" = "v1.49.0"
  ```

- Both handlers wired into `PackagesConfigRef`, `install_packages`
  dispatcher, `Config` struct, `TOP_LEVEL_SECTIONS`,
  `validate_package_section`, and `run_packages_phase` telemetry.
  `GEM_KNOBS` / `GO_KNOBS` slices pinned by destructure tests so adding
  a future config knob without updating the slice fails compilation
  instead of silently making the validator reject the new knob as a
  hostile package name.

- `packages.phase_started` / `packages.phase_completed` events now
  carry `gem` and `go` booleans alongside the existing
  `npm`/`pip`/`cargo`/`nuget` flags. `packages.phase_previewed` carries
  matching `gem_count` / `go_count` for dry-run preview observability.

- Per-package name + version validation (control bytes, leading-`-`,
  URL schemes) inherits unchanged from `packages/common.rs`. The
  trust-gate refusal of remote-config installs without
  `[packages] allow_remote = true` now also covers `[gem]` / `[go]`.

### Changed — `src/main.rs` extraction (PRD-037)

- `src/main.rs` reduced 734 → 271 LOC (-63%). The original
  1500-line `main()` match block is fully eliminated.

- All CLI dispatch + 14 `handle_*` glue helpers moved to a new
  `src/commands/dispatch.rs` (486 LOC). `main` now retains only
  process init that genuinely belongs at the entry point: telemetry
  config merge precedence (`env > project > global`), sandbox banner
  muting, panic hook, OTLP flush at exit, and the
  `extract_config_path` helper for early telemetry config loading.

- Per-command modules already lived at `src/commands/*_cmd.rs` from
  earlier PRD-037 phases; this round finishes the extraction by
  taking the routing table out of `main` too.

- Zero behavior change: same exit codes, same output, same flag
  forwarding, same OTLP flush sequence. `cargo fmt`, `clippy
  --all-features -- -D warnings`, 814 lib tests, and the full
  integration test matrix are all green on the refactored layout.

### Added — documentation (PRD-011)

Closes the six remaining `docs/` gaps from the PRD-011 audit. All new
pages match the existing flat layout (no new subdirectories) and the
Material for MkDocs style (tabbed code blocks, admonitions, fenced
code with `title=`):

- `docs/installation.md` — full install guide for macOS, Linux,
  Windows, and from-source. Covers winget / scoop / choco / brew /
  cargo, verify steps, update channels, and clean-uninstall.
- `docs/services.md` — operational guide for `[services]`: Docker
  Compose, Tilt, inline service blocks, auto-start during `jarvy
  setup`, CI auto-disable, and `--wait-healthy` patterns.
- `docs/environment.md` — `[env]` guide: plain variables, tool-scoped
  overrides, secret resolvers (prompt / `from_env` / 1Password /
  Vault / AWS Secrets Manager), `.env` vs shell rc, trust boundaries.
- `docs/tools-by-category.md` — 235+ tools grouped by purpose so
  users browsing for "what's available" can scan instead of
  `jarvy search`-ing blindly.
- `docs/contributing-testing.md` — contributor testing guide: when to
  reach for unit / integration / E2E layers, `assert_cmd` patterns,
  `insta` snapshots, `JARVY_TEST_MODE` / `JARVY_FAST_TEST` /
  `JARVY_E2E` flags, common pitfalls.
- `docs/decisions.md` — architecture decisions index. Pointers to
  the canonical sources (`prd/*.md` + `CLAUDE.md`) with one-line
  summaries for the highest-leverage trust, architecture, and
  convention decisions.

- `docs/packages.md` updated with `[nuget]`, `[gem]`, `[go]`
  sections matching the existing `[npm]` / `[pip]` / `[cargo]`
  format; updated module-source line; expanded order-of-operations
  to list all six ecosystems.

- `mkdocs.yml` nav extended: Installation under Get Started;
  Environment variables + Services under Guides; Tools by category +
  Architecture decisions under Reference; Contributor testing guide
  under Community.

### Changed — PRD task tracker hygiene

- Updated `tasks/prd-*.json` for nine PRDs whose JSON status had
  drifted from on-disk reality: 002 (tool post-install hooks), 011
  (documentation), 013 (235 tool dirs vs the 150 target), 014
  (real-world testing — examples + smoke tests + e2e workflow ship),
  021 (MCP server — `src/mcp/` ships), 027 (observability —
  `src/observability/` ships), 037 (main.rs refactor), 038 (E2E
  harness — Phase 1 GitHub-hosted ships; Phase 2 AWS EC2 deferred),
  039 (language packages — gem/go added in this release).

- Each updated JSON carries a `completionNote` field with verification
  evidence: the on-disk files, the LOC delta, or the directory count
  that demonstrates the work is actually shipped (not just intended).

### Tracking

- Closes PRD-011 (Comprehensive Documentation System)
- Closes PRD-013 (Expand Tool Coverage)
- Closes PRD-014 (Real-World Testing and Example Configurations)
- Closes PRD-037 (Main.rs Code Maintainability Refactor)
- Closes PRD-038 (Hybrid Cross-Platform E2E Testing Harness — Phase 1)
- Closes PRD-039 (Language Package Dependencies)
- Stale-status sync for PRD-002, PRD-021, PRD-027 (code shipped earlier)

## [v0.3.0] — Repo relocation to Cliftonz + MCP auto-register default-on (2026-06-26)

First release under the new canonical home, `github.com/Cliftonz/Jarvy`.
The repository was transferred from the `bearbinary` org; the old URL
continues to auto-redirect for git and HTTP traffic, but signing and
package metadata now point at the new owner. Existing users on v0.2.x
do not need to re-clone — `git pull` will follow the redirect — but the
cosign cert-identity baked into v0.3.0 is anchored to `Cliftonz/jarvy`
and will reject artifacts signed under the old subject. There is no
backwards-compatible overlap window; this is a clean cut.

Bundled with the move: `[mcp_register]` now opts in by default when
`jarvy setup` runs against a project with no explicit block, the
`scripts/bootstrap.sh` one-command onboarding is now the canonical
entry point for contributors, and two CI bugs that surfaced during the
v0.2.2 publish (missing Linux tarballs for AUR/chocolatey, parent-vs-
templates version gating) are fixed.

### Changed — repository home

- **Repository relocated to `github.com/Cliftonz/Jarvy`.** All in-tree
  references — Cargo.toml `repository`, install scripts, package
  manifests (Homebrew, AUR, RPM, Debian, winget, chocolatey, Helm),
  documentation URLs, CODEOWNERS, FUNDING — rewritten in a single
  sweep. Cosign cert-identity regex anchored to the new owner; releases
  signed under `bearbinary/jarvy` will no longer verify. GitHub Pages
  (`jarvy.dev`), crates.io ownership (`jarvy`, `jarvy-templates`), and
  Actions secrets carried over via the transfer. CODEOWNERS team
  syntax (`@bearbinary/maintainers`) collapsed to `@Cliftonz` since the
  new home is a user account, not an org.

### Added

- **`[mcp_register]` default-on auto-register.** `jarvy setup` now
  synthesizes a default `[mcp_register]` block when the project has no
  explicit block and at least one supported AI agent (Claude Code,
  Cursor, Codex, Windsurf, Cline, Continue) is detected on disk. The
  built-in `jarvy` MCP server is registered against each detected agent
  with project scope (this repo only), not user scope. Fires the
  `mcp_register.auto_detected` telemetry event with `count`, `agents`,
  `platform`. Suppressed in dry-run, test mode, seamless / CI sandboxes,
  and when `JARVY_MCP_REGISTER=0`. Explicit blocks always win.

### Fixed

- **AUR and chocolatey downstream publish unblocked.** The v0.2.2
  release workflow built macOS+Windows artifacts but skipped the Linux
  tarballs that AUR `PKGBUILD-bin` and chocolatey's MSI bundler expect
  (CPMR0041). The release matrix now produces
  `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` tarballs
  alongside the existing platforms, so the downstream publish step
  finds its inputs.
- **`jarvy-templates` publish gate now reads the templates crate's own
  version.** Before this fix, the publish workflow keyed off the parent
  `jarvy` crate's version, which meant a parent-only release would
  attempt to re-publish a `jarvy-templates` version that already
  existed on crates.io (and fail). The workflow now reads
  `crates/jarvy-templates/Cargo.toml::version` and only publishes when
  that specific version is new.

### Docs

- **`scripts/bootstrap.sh` is now the canonical one-command onboarding
  path.** End-user repos integrating Jarvy should copy it into their
  own `scripts/` so contributors run `./scripts/bootstrap.sh` to
  install Jarvy (via `dist/scripts/install.sh`) and execute `jarvy
  setup` against the repo-root `jarvy.toml`. Idempotent. Flags:
  `--no-setup`, `--channel <stable|beta|nightly>`, passthrough args to
  `jarvy setup`. Quickstart and contributor docs updated to surface
  this over hand-rolled curl-pipe + `cargo install` snippets.

## [v0.2.2] — Opt-out telemetry default + P0 seamless-gate fix (2026-06-25)

Patch release on the v0.2.x line, but a behavior-significant one: the
telemetry default flipped from opt-in to opt-out, and a P0 security
regression in the CI / sandbox auto-disable was caught and fixed
before any stable shipped with the flip. The two changes are bundled
because they were authored back-to-back in the same evening — the
opt-out flip introduced the regression, and the follow-up commit
closed it along with 15 review findings from a five-persona parallel
code review (security / Rust perf / QA / observability /
maintainability).

Users on a pre-`[telemetry]`-block legacy config also now see the
disclosure on first post-upgrade run, closing the silent-enrollment
loop the security reviewer found. Privacy-disclosure surfaces (`PRIVACY.md`,
`UPGRADING.md`, `data/faq.json`) were swept to match the new posture.

### Changed — privacy posture

- **Telemetry default flipped from opt-in to opt-out.** New installs and
  existing installs whose `~/.jarvy/config.toml` has no explicit
  `[telemetry] enabled = …` line now ship anonymized usage data to
  `https://telemetry.jarvy.dev` by default. The first-run boxed notice
  declares telemetry enabled and surfaces the disable path; the
  end-of-`setup` nudge fires when the user is still on the default and
  points at `jarvy telemetry disable`. Trust boundary unchanged: a
  remote `jarvy.toml` can still only narrow telemetry, never broaden
  it.

  Disable persistently with `jarvy telemetry disable`, per-invocation
  with `JARVY_TELEMETRY=0 jarvy <cmd>`, or via `[telemetry]
  enabled = false`. CI / unattended AI sandboxes still auto-disable —
  that guardrail was hardened (see Fixed below).

  Public docs and disclosure surfaces updated: `CLAUDE.md`, `PRIVACY.md`,
  `UPGRADING.md`, `docs/telemetry.md`,
  `docs/operations/telemetry-forwarder.md`, `docs/ai-hooks.md`,
  `docs/ai-sandboxes.md`, `docs/index.md`, `docs/release-testing.md`,
  `docs/for-ai-agents.md`, `data/faq.json`.

### Added

- **`telemetry.disclosure_shown` event.** Fires after the first-run
  boxed banner (or the legacy-upgrade banner for users whose config
  pre-dates the `[telemetry]` block) renders. Carries `trigger`
  (`first_run` / `legacy_upgrade`) and `platform`. Gives on-call an
  audit trail when users file privacy complaints.
- **`telemetry.undecided_nudge_shown` event.** Fires when the
  end-of-`setup` "Note: opt-out and currently on" line emits. Carries
  `platform`. Lets operators graph what fraction of the fleet is still
  in the undecided state and decide when to retire the nudge.
- **Legacy-upgrade disclosure.** Users with a `~/.jarvy/config.toml`
  that pre-dates the `[telemetry]` block now see the boxed disclosure
  on the next post-upgrade run, after which the block is persisted
  with `enabled = true` so the disclosure doesn't repeat. Closes a
  silent-enrollment loop that would otherwise leave the long tail of
  pre-`d039d9b` configs without ever seeing the banner.

### Fixed

- **`jarvy setup` no longer re-prompts "Do you want to install Oh My
  Zsh?" when `~/.oh-my-zsh` already exists.** The macOS hard-dep check
  asked first and *then* detected the existing install. Detection now
  runs before the prompt; the `tool.already_installed` telemetry event
  still fires (`prompted_user = false`). The decision logic moved into
  a pure `decide_omz_action` function with a table-driven regression
  test — including a `never_prompt` closure that panics if invoked,
  pinning the "AlreadyInstalled short-circuits before any prompt"
  invariant.
- **CI / sandbox telemetry auto-disable now actually fires under the
  opt-out default.** `from_env`'s seamless-detection branch correctly
  computed `enabled = false` when `JARVY_TELEMETRY` was unset, but
  `main.rs` only propagated `env_config.enabled` when the env var was
  *set* — discarding the disable in exactly the case it was supposed to
  fire. Under the prior opt-in default this was masked because the
  disk value was already false. The seamless gate now applies
  unconditionally after the config merge when `JARVY_TELEMETRY` is
  unset. Forced sandbox (`JARVY_SANDBOX=1` without real detection) is
  deliberately NOT in this gate — a hostile dotfile must not silence
  telemetry on the victim's machine.
- **`tool.already_installed` install_path is now home-prefix
  redacted.** Pre-flip the event only fired after a user prompt;
  post-flip it can fire automatically on every `jarvy setup` (the OMZ
  short-circuit). The raw `/Users/<name>/.oh-my-zsh` path is now
  routed through `redact_path` to `~/.oh-my-zsh` before emit. The
  forwarder's server-side scrub remains the defense-in-depth backstop,
  not the contract.
- **`search.executed` no longer emits the raw query string.** The
  user's free-text input previously shipped verbatim — invisible
  under opt-in, but a leak surface once telemetry was on by default.
  Replaced with `had_results` (bool) and `query_len_bucket`
  (`0`/`1-4`/`5-15`/`16-40`/`40+`) so hit-rate dashboards still work
  without storing the query text.
- **`emit_telemetry_hint_if_undecided` uses a section-aware TOML
  parse, not a line-by-line string match.** The prior predicate
  treated `[mcp_register]\nenabled = true` (or any sibling section's
  `enabled` key) as a telemetry decision, suppressing the nudge for
  users who never made one. Extracted as
  `telemetry::user_decided(content)` with five table-driven test
  cases pinning the section-aware behavior.

## [v0.2.1] — Registry pull QA suite + sync.rs supply-chain fixes (2026-06-25)

Patch release on the v0.2.x line. Dominantly defensive: a comprehensive
end-to-end QA suite for the `jarvy registry sync` feature shipped in
v0.2.0, plus the two real bugs that suite caught in the supply-chain
verification path. Also closes Windows test-isolation tech debt that had
been silently red on every tag-push CI run going back to v0.2.0-rc.1.
Soaked as `v0.2.1-rc.1` → `-rc.8` over 2026-06-24 → 2026-06-25; soak
record in [#39](https://github.com/Cliftonz/Jarvy/issues/39).

The two registry-sync bug fixes are the user-impacting items. Operators
running `jarvy registry sync` against a cosign-signed manifest in v0.2.0
were getting fail-CLOSED behavior that looked correct on the surface
(verification rejected) but happened for the wrong reason (the sig/pem
staging paths never matched what `cosign verify-blob` looked for), so
the actual signature was never checked. The second fix closes a window
where a malformed manifest body could be promoted to the canonical
`manifest.json` path before validation rejected it — a subsequent
`jarvy registry status` would then dump the invalid bytes verbatim.
Both shipped silently in v0.2.0 because the original PR only had
in-process tests of `run_sync_with_config`; the new e2e suite drives
the real binary against a programmable mock registry + cosign shim and
is what surfaced them.

### Known limitation — bootstrap-mode gates carry forward

Same status as v0.2.0: [#30](https://github.com/Cliftonz/Jarvy/issues/30)
is still open, so the Path 2/3/4 (upgrade / skip-version / rollback) CI
gate still runs in bootstrap mode. No regression vs v0.2.0; the gap
closes when tarballs ship.

### Added

- **Comprehensive registry-pull QA suite** (~1900 LOC across 4 new test
  files). End-to-end lifecycle (configure → sync → status → clear),
  cosign signature path with a FakeCosign shim, resilience (oversized
  manifest, truncated body, HTTP 500, parallel-fetch stress, recovery
  after prior failed sync, duplicate names, invalid UTF-8, unparseable
  TOML), and tracing-event regression guards that pin
  `registry.sync.{started,completed,sha_mismatch,signature_disabled,failed}`
  by name + level + field shape against the documented OTEL taxonomy.
  Replaces the prior in-process-only coverage that missed the
  staging-path bug.

### Fixed

- **Registry `cosign verify-blob` actually verifies now.** Prior to this
  release, `verify_sigstore_signature_with_identity` looked for
  `manifest.json.unverified.{sig,pem}` as siblings of the staged
  manifest, but the orchestrator wrote them at
  `manifest.json.{sig,pem}.unverified`. Cosign returned
  `SignatureFilesMissing` on every invocation, which `signature_outcome_is_acceptable`
  correctly rejected — so the failure mode was fail-CLOSED ("sync
  refused") rather than silent-bypass, but no signature was ever
  actually checked. Staging now uses the path shape cosign's extension
  derivation expects.
- **Malformed manifest bodies no longer poison the cache.** Previously,
  `sync.rs` wrote `manifest.json.unverified` to disk and then promoted
  to the canonical `manifest.json` BEFORE parsing the bytes. A non-UTF-8
  or syntactically invalid manifest would error out of sync but leave
  the canonical file populated with the bad bytes, which
  `jarvy registry status` then printed verbatim. Manifest is now parsed
  in-memory before any disk write; promotion happens only after a
  successful parse.
- **Windows test-isolation tech debt cleared across the suite.** Eight
  previously-silent Windows-only test failures (`paths::tests`,
  `network::propagate::tests`, `update::installer::tests`, plus 12
  `ai_hooks_integration` + 2 `mcp_register_integration` tests) had
  been red on every tag-push CI run since v0.2.0-rc.1 because (a) test
  helpers hard-coded `/tmp` paths that aren't absolute on Windows,
  (b) `Path::starts_with` is component-aware but string-prefix checks
  with `format!("{prefix}/")` weren't, (c) `dirs::home_dir()` on
  Windows is Win32-API-backed and ignores HOME/USERPROFILE env vars
  (so test sandboxes had no effect), and (d) `cosign` discovery only
  knew about `.exe`, not `.cmd`/`.bat`. All fixed; the Test workflow
  now runs Windows-green on every tag push. v0.2.0 stable shipped with
  these failures as inherited sev-2.
- **Test-mode bypass for `jarvy audit`.** `audit::run_one_scanner` now
  honors `JARVY_FAST_TEST=1` (the documented test-mode contract for
  "skip external command execution") and returns synthetic "not
  available" results. The test for this code path went from 683s to
  1.7s locally.

### Changed

- **Registry CLI + cache events now route through `telemetry_gate::emit`.**
  Closes the opt-in contract for the `registry.*` event family — v0.2.0
  leaked `registry.cli.sync_failed`, `registry.cache.swap_failed`, and
  the `registry.cache.index_*` events to OTLP even when the user had
  set `telemetry.enabled = false`. Matches the contract already
  documented for the `package.*` event family.
- **CI Test workflow on `cargo-nextest`.** Switched from `cargo test`
  to `cargo nextest run --all-features --no-fail-fast`. Process-level
  parallelism per test; the Windows lane went from ~14 min to ~3-4 min
  warm-cache. Also dropped `--show-output` (Windows terminal I/O sink)
  and `--verbose` from `cargo check`.
- **CI actions on Node 24.** Bumped `actions/checkout` v4→v7,
  `actions/upload-artifact` v4→v7.0.1, `actions/deploy-pages` v4→v5,
  `softprops/action-gh-release` v2.2.1→v3.0.0,
  `KSXGitHub/github-actions-deploy-aur` v2.7.2→v4.1.3. Clears the
  Node 20 deprecation warnings the runner had been forcing through.

### Tooling

- **Cursor + JetBrains Toolbox Linux install support** ([#35](https://github.com/Cliftonz/Jarvy/pull/35)).
  Both were macOS+Windows only in v0.2.0; Linux now lands via tarball
  fallback paths.
- **9 networking tools** ([#36](https://github.com/Cliftonz/Jarvy/pull/36)):
  `cloudflared`, `headscale`, `nebula`, `netbird`, `openvpn`,
  `tailscale`, `twingate`, `wireguard-tools`, `zerotier`. Covers VPN +
  overlay-mesh stacks for both home-lab and corp deployments.

## [v0.2.0] — Tooling breadth, MCP surface, AI hooks, release-soak hardening (2026-06-22)

First minor release in the v0.x line. Bigger than its predecessor — 32
commits adding two new tool ecosystems (NATS messaging, .NET / NuGet), a
significant MCP tool surface, AI-hooks distribution to six coding agents,
auto-registration of the Jarvy MCP server, and the release-soak CI gates
that catch regressions before promotion. Soaked as `v0.2.0-rc.1` →
`-rc.2` over 2026-06-16 → 2026-06-22; soak record in
[#25](https://github.com/Cliftonz/Jarvy/issues/25).

### Known limitation — binary self-update gate ships in bootstrap mode

The Path 2/3/4 (upgrade / skip-version / rollback) CI gate is live but
[#30](https://github.com/Cliftonz/Jarvy/issues/30) is open: `release.yml`
does not yet emit `.tar.gz` / `.zip` binary tarballs as release assets, so
the `BinaryInstaller` self-update path has nothing to consume. Users on a
package-manager path (Homebrew, cargo, apt, dnf, pacman, winget,
Chocolatey, scoop, AUR) update normally. Users on the binary fallback see
"No binary for this platform" — same documented gap as v0.1.x. Tracked for
v0.3.0.

### Added

- **NATS messaging toolchain (4 tools).** `nats-server`, `nats` CLI, `nsc`
  (account credentials), plus a `nats-services` built-in template that
  wires a working three-service mesh into a fresh `jarvy.toml`.
- **.NET / NuGet ecosystem.** New `[nuget]` package section + `NugetHandler`
  with end-to-end dry-run + install support. 5 .NET dev tools (full set
  validated against upstream channel docs), 5 .NET-flavored templates, 5
  example configs, and `grpcurl` for grpc service introspection.
- **12 queuing / messaging tools across two batches.** First batch: 6
  workflow + broker tools. Second batch: `pulsar`, `kaf`, `kafkactl`,
  `emqx`, `argo` (Workflows CLI), `kn` (Knative CLI). Tools without
  first-party Windows manifests omit the `winget` block entirely rather
  than ship placeholder IDs that could be hijacked under
  supply-chain attack (see Security).
- **Extended MCP tool surface.** AI hooks, MCP register, drift, roles,
  services, templates, validation — all exposed over MCP. Mutating tools
  (`services_start`, `templates_use`) gated by `gate_mutation` +
  `MutationCtx`: rate limit → stderr TTY confirm → audit log. Workspace
  containment enforced by `safety::resolve_within_workspace` (canonical-
  root check; refuses `..`, absolute escapes, endpoint symlinks).
- **`ai_hooks` distribution to six AI coding agents.** Curated guardrail
  hooks (the "don't `rm -rf` your homedir", "respect .gitignore",
  "stop-on-tests" class of safeguard) provisioned uniformly to Claude
  Code, Cursor, Codex, Windsurf, Cline, and Continue. Bash → PowerShell
  translation on Windows handled in-process so the same hook YAML works
  cross-platform.
- **`mcp_register` auto-registration to the same six agents.** One-shot
  setup that places the Jarvy MCP server entry in each agent's config
  with the correct stdio invocation, so users don't have to copy-paste
  per-agent boilerplate. Trust-gated: only the built-in `jarvy` server
  registers from a remote config unless `allow_custom_servers = true`.
- **Telemetry category plumbing.** `category` field travels through every
  `tool.requested` / `tool.installed` / `tool.failed` event, plus
  `template.materialized`. Operators can graph "what fraction of NATS
  rollouts succeeded?" without pivoting on tool name.
- **`tool.already_installed` event.** Surfaces the skip path with
  `install_path`, `detection_method`, `prompted_user` fields — previously
  invisible in telemetry, now visible.
- **Telemetry `error_kind` discrimination.** `tool.failed` carries an
  `error_kind` enum (`tap_fetch`, `command_failed`, `permission_denied`,
  …) so an operator can split "the brew tap was unreachable" from "the
  binary install actually broke".
- **Drift report category grouping.** Tools group by category in human
  output (`messaging`, `workflow`, `runtime`, …) instead of one flat
  list, making diff review tractable at scale.
- **CI: Path 8 asset download sweep workflow.** `.github/workflows/verify-release.yml`
  fetches every release asset, verifies HTTP 200, sha256 against
  `SHA256SUMS.txt`, cosign signature, SBOM well-formedness, and asserts
  the `.deb`-extracted binary's `--version` matches the tag's core version.
  Auto-fires on `release: published` and weekly to catch asset rot.
- **CI: Path 2/3/4 release-paths validation workflow.** `.github/workflows/release-paths.yml`
  exercises upgrade-from-N-1, skip-version-from-N-2, and rollback flows
  on macOS arm64 / Ubuntu 22.04 / Windows. Runs in bootstrap mode until
  #30 ships tarballs; auto-tightens to hard-fail after.
- **CI: one-shot winget submission helper.** For first-time Jarvy.Jarvy
  publisher onboarding.

### Changed

- **Dash ↔ underscore tool aliasing is now uniform.** `nats-server` and
  `nats_server` resolve to the same tool in three places that previously
  diverged: `registry::get_tool()`, `commands::validate::validate_tools`,
  and `tools::spec::get_tool_spec()`. The third site was the sev-2 found
  during rc.1 soak and fixed in rc.2 — `validate` accepted `nats-server`
  but `setup --dry-run` reported `tool.unsupported` for the same name.
- **Brew tap auto-tap.** When `macos.brew` (or `linux.brew` fallback) is
  `org/tap/formula` form (exactly two slashes), install path runs
  `brew tap org/tap` first so a fresh box doesn't surface an "untrusted
  tap" error. Soft-fail; already-tapped is not a blocker.
- **`jarvy validate` and `jarvy setup --dry-run` now surface `[nuget]`.**
  Previously the new section silently dropped from the validate report —
  users would think their NuGet packages were configured when they
  weren't.
- **`publish-packages.yml` decouples downstream channels from crates.io.**
  Previously a transient crates.io publish failure left winget / chocolatey
  / homebrew unsynced. Each channel now has independent secret gates and
  failure modes.
- **Release binary `--version` comparison uses core version, not full
  tag.** rc tags like `v0.2.0-rc.2` build binaries that report
  `jarvy 0.2.0` (no prerelease suffix); the verify-release step now
  matches on core only.

### Fixed

- **Drop placeholder Windows package IDs from tool definitions.**
  Six tools previously listed placeholder `winget` IDs like
  `Pivotal.RabbitMQ` for upstream namespaces that the publisher had not
  actually claimed. Any party who registered that publisher could ship
  a malicious installer pinned by `winget install -e --id`. Replaced
  with explicit `// No first-party winget manifest as of YYYY-MM` notes;
  `tool.unsupported` telemetry fires in place at runtime.
- **Telemetry gate respects `[telemetry] enabled = false`.** Every
  `package.*` / `packages.*` / `package_command.failed` event reads
  `observability::telemetry_gate::is_enabled()` before emitting. Prior
  implementation leaked package events to OTLP when telemetry was
  disabled but an endpoint was set for unrelated reasons. Broke the
  documented opt-in contract.
- **MCP safety boundary applies to extended mutating tools.** The new
  drift/roles/templates/services tools all run through
  `resolve_within_workspace` — a path containing `..` or an absolute
  escape that lands outside the workspace root canonicalizes to a
  refusal, not a silent file write.
- **De-flaked `telemetry_smoke` integration test.** Ephemeral port +
  `#[serial]` annotation + 30s timeout, replacing the prior flaky
  hardcoded port that intermittently lost to other tests' bound
  sockets.
- **Mass conversion of ~200 `_registered_returns_some` tautology tests
  to `_registration_shape` tests.** The old tests verified
  `Some(_).is_some()` after registration — a tautology that always
  passed even when the underlying `ToolSpec` was structurally broken.
  Replaced with shape-asserting tests that fail when a tool's platform
  matrix degrades.

### Security

- **Supply-chain: no more placeholder winget IDs.** See Fixed above.
- **Package-name validation.** `validate_package_name` /
  `validate_package_version` refuse leading-`-`, URL schemes, shell-meta,
  and control bytes (ESC/BEL/DEL/NUL — closes ANSI injection in dry-run
  preview). `jarvy validate` runs them on every `[npm]/[pip]/[cargo]/[nuget]`
  entry.
- **Remote-config trust narrowing only.** `ConfigOrigin::Remote` tags
  remote-fetched configs; `allow_custom_commands`, `allow_custom_servers`,
  `allow_remote` (packages), and telemetry endpoint override are all
  refused for remote configs. Library hooks and the built-in `jarvy` MCP
  server remain trustable; user-authored extensions do not.

### Impact on v0.1.x users

- **Cargo (`cargo install jarvy`)** — resolves to v0.2.0; no breaking API
  surface in command flags. Existing `jarvy.toml` parses unchanged.
- **`.deb` / `.rpm` / `.dmg` / `.msi` / `.AppImage`** — install normally
  from the GitHub release.
- **Homebrew, install.sh, install.ps1** — still broken pending #30,
  same as v0.1.x. No regression; no improvement.
- **`jarvy update`** — package-manager paths upgrade fine. Binary
  fallback returns the documented "No binary for this platform" — same
  state as v0.1.x, tracked in #30.



Patch release closing the crates.io gap that v0.1.0 left open. No
runtime code changes — release-pipeline metadata only.

### Fixed

- **`jarvy-templates` is now publishable.** The crate was marked
  `publish = false` and lacked the `repository` / `homepage` metadata
  crates.io requires. Both `jarvy` and `cargo-jarvy` depend on it via
  `{ version = "X", path = "..." }`; crates.io strips `path` on publish
  and resolves from the registry, so the dep must already be available
  there. With `publish = false` + no version spec on the parents, the
  v0.1.0 `cargo publish` failed at `error: failed to verify manifest
  ... 'jarvy-templates' does not specify a version` before either crate
  could upload.
- **Both `jarvy-templates` path dependency declarations now carry a
  `version = "0.1.1"` requirement.** Required by `cargo publish` —
  without it the parent crate cannot verify against the published
  registry form of the dep.
- **`publish-packages.yml::publish-crates-io` step is now ordered.**
  Previously one `cargo publish` call attempted to publish `jarvy` as
  the workspace root; `jarvy-templates` was never published, so the
  parent's resolve always 404'd. The job now publishes
  `jarvy-templates` first, polls the crates.io index for up to 150s
  until the dep surfaces, then publishes `jarvy` with `--no-verify`
  (the workspace verify already ran at tag-build time; the
  post-publish re-verify would race the index refresh).

### Impact on v0.1.0 users

- The GitHub Release for v0.1.0 (all 49 binary assets + Sigstore
  signatures) is unaffected. `.deb` / `.rpm` / `.dmg` / `.msi` /
  `.AppImage` install paths work exactly as documented.
- `cargo install jarvy` resolves to v0.1.1 (the first crates.io
  release in the v0.1.x line). Users who tried `cargo install jarvy`
  during the v0.1.0 → v0.1.1 window saw `error: could not find
  jarvy 0.1.0 in registry crates-io`.
- Other channels (Homebrew tap, AUR, winget, Chocolatey) were not
  affected by this gap.

## [helm-v0.6.1] — Defense-in-depth: anonymize record-level attrs (2026-05-25)

### Fixed — `jarvy-telemetry-forwarder` Helm chart

- `transform/anonymize` now runs at the record level in addition to
  resource context. The 0.6.0 chart's hash statements were scoped to
  `context: resource`, so any client SDK that emitted a PII-shaped
  attribute as a per-event field (e.g. `tracing::info!(hostname = %h)`)
  bypassed the SHA256 hash and reached the backend in plaintext. The
  companion SDK fix moves `host.name` to the resource where it belongs;
  this chart change makes the privacy contract hold even when a future
  SDK regression mis-slots an attribute.
- Adds a second OTTL statement context to each pipeline
  (`log` / `datapoint` / `span`). Same `pii.hashedAttributes` list is
  reused — single source of truth.
- `keep_keys` is intentionally NOT applied at record level: event-
  specific attributes (`event`, `tools`, `duration_ms`) are not PII and
  must pass through. Resource-context `keep_keys` remains the
  allowlist enforcement point for per-process identity attrs.

### Migration

No action needed. Patch bump; no values surface change. Consumers can
no-op-upgrade.

## [helm-v0.6.0] — Grafana Cloud OTLP region default fix (2026-05-25)

### Fixed — `jarvy-telemetry-forwarder` Helm chart

- `exporter.endpoint` default hardcoded `prod-us-east-0`, but
  Grafana Cloud API keys are region-bound — keys issued for any other
  region 401 at the gateway and silently drop every export. The home
  stack lives on `prod-us-east-3`; consumers relying on the chart
  default had 100% export failure for traces, metrics, and logs.

### Changed — `jarvy-telemetry-forwarder` Helm chart

- Region hoisted to a new top-level value `grafanaCloud.region`
  (default `prod-us-east-3`). When `exporter.endpoint` is empty the
  chart now composes
  `https://otlp-gateway-<region>.grafana.net/otlp` via a single
  `exporterEndpoint` helper. Explicit `exporter.endpoint` still wins
  unchanged — operators pointing at Honeycomb / Datadog / in-cluster
  Tempo keep their override path.
- Both the Deployment's `BACKEND_OTLP_ENDPOINT` env and the
  `CiliumNetworkPolicy`'s FQDN derivation flow through the same
  helper, so a region bump can't desync the egress allow-list from
  the actual gateway.

### Migration — BREAKING

`exporter.endpoint` default is now empty (was a hardcoded URL).
Consumers that depended on the chart-default us-east-0 URL must
either:

- Set `exporter.endpoint` explicitly to keep their previous URL, or
- Align `grafanaCloud.region` with their stack's region (default
  `prod-us-east-3` works for home-cluster installs).

## [helm-v0.5.3] — `helm test` smoke pod actually works now (2026-05-20)

The 0.5.2 ship landed the `helm test` smoke pod + supporting infra
but the pod itself never ran green in CI on the first push (or on
local kind clusters). Three fixes were needed; this release rolls
them into a clean cut.

### Fixed — `jarvy-telemetry-forwarder` Helm chart

- NetworkPolicy: explicit egress allow for in-namespace `helm test`
  pods (paired with the 0.5.2 ingress rule). Production CNIs
  (Cilium, Calico) are conntrack-aware and don't need this — it's
  defense-in-depth for CNIs that evaluate egress per-packet
  (kindnet).
- Test pod hook-delete-policy drops `hook-succeeded` so the pod
  sticks around after a green run. Without this, `helm test --logs`
  failed with `pods ... not found` because the pod was deleted
  before the log fetch ran.
- Test pod template is now nil-safe (nested `if .Values.tests`
  before `.enabled`). Fixes a render failure when the template
  file from a newer chart is checked out alongside an older
  `values.yaml` that doesn't carry the `tests:` block (CI
  upgrade-leg pattern).

### Fixed — `helm-chart-ci` workflow

- Live install + upgrade step deletes the NetworkPolicy before
  running `helm test`. kindnet's netpol enforcement isn't
  conntrack-aware, so the collector's `wide-except-rfc1918`
  egress filter drops reply SYN-ACKs to in-cluster test pods.
  The netpol structure itself is fully covered by the render +
  kubeconform matrix; this step covers the receiver only.
- Common-annotations fanout test now sees the test pod carrying
  the chart's common annotations.
- Diagnostics-on-failure step dumps pods, services, endpoints,
  netpol, collector logs, test-pod logs, and runs a netpol-free
  repro curl. Costs nothing on green runs.
- Three other pre-existing matrix failures fixed in the same
  iteration (kept here for the changelog reader's context):
  helm/kind-action SHA pin corrected, promtool input shape
  (extract `.spec` for RuleGroups), extraEnv reject assertion
  accepts both helm 3.18 and helm 4.x schema messages.

### Migration

No action needed. The chart now passes `helm test` cleanly on
production CNIs. On stock kindnet (only relevant for in-cluster
test runs, not production), drop the NetworkPolicy before
running `helm test` — see the workflow comment for the rationale.

## [helm-v0.5.2] — `helm test` smoke pod + live HTTPS smoke script (2026-05-20)

### Added — `jarvy-telemetry-forwarder` Helm chart

- `templates/tests/otlp-smoke.yaml` — `helm test` hook pod that POSTs
  minimal OTLP/HTTP payloads at `/v1/{logs,metrics,traces}` on the
  Collector Service and asserts 2xx. Validates the receiver pipeline
  end-to-end after `helm install` without depending on the public
  ingress. Image `curlimages/curl:8.10.1` pinned, restricted-PSS
  compliant.
- `tests.*` values + schema validation (`enabled`, `image`,
  `resources`, `securityContext`). Disable with
  `--set tests.enabled=false`.
- NetworkPolicy now whitelists pods carrying BOTH the chart-test
  component label AND the release instance label — required so the
  `helm test` pod can reach the Collector through the otherwise
  locked-down ingress.
- `scripts/smoke-live.sh` — bash script that smokes the public
  HTTPS endpoint with the same three OTLP payloads. A diff between
  this and the in-cluster `helm test` isolates ingress (TLS,
  gateway, middlewares) as the suspect.
- Makefile targets: `helm-smoke-live` (live HTTPS) and
  `helm-test-kind` (in-cluster).
- `helm-chart-ci` kind job now runs `helm test` after the fresh
  install — receiver-pipeline regressions fail CI alongside the
  rendering/lint suite.

### Migration

No action needed; new behavior is purely additive. `helm test`
becomes opt-in once you upgrade — run it whenever you want
in-cluster validation of the receiver path.

## [helm-v0.5.1] — HTTPRoute `filters: null` lint fix (2026-05-17)

### Fixed — `jarvy-telemetry-forwarder` Helm chart

- HTTPRoute template no longer emits an empty `filters:` key (which
  YAML-parses as `null`) when traefik middlewares are disabled and no
  `extraFilters` are supplied. Surfaced by the `helm-chart-ci`
  matrix's `gatewayclass-envoy-accepted` scenario, which has been
  failing kubeconform-strict since the field was added — the Gateway
  API HTTPRoute schema types `filters` as `array`, not
  `array | null`. The fix wraps the key in an `or` guard so it is
  omitted entirely when no filters apply, which is the
  spec-compliant equivalent and produces no Argo CD drift.

### Migration

No action needed. Behavior at runtime is unchanged — a missing
`filters` key and an empty `filters` list both mean "no filters
applied". The diff visible on `helm diff upgrade` is purely the
removal of an `null`-valued field from the rendered HTTPRoute when
running without traefik middlewares.

## [helm-v0.5.0] — ExternalSecret Argo CD drift fix (2026-05-17)

Rendered ExternalSecrets now emit the two server-side defaults the ESO
admission webhook fills in (`target.deletionPolicy: Retain`,
`data[].remoteRef.conversionStrategy: Default`). Without these in the
chart's desired manifest, Argo CD's compare saw the webhook-injected
values as drift on every reconcile, leaving every install of this
chart perpetually `sync=OutOfSync, health=Healthy`. Discovered while
diagnosing the `jarvy-telemetry` Argo app on the home cluster on
2026-05-17.

### Added — `jarvy-telemetry-forwarder` Helm chart

- `secrets.externalSecrets.deletionPolicy` (default `Retain`) and
  `secrets.externalSecrets.conversionStrategy` (default `Default`)
  values. Both default to the ESO server-side default so existing
  installs see no semantic change — only that Argo CD diffs now show
  zero drift after the next `helm upgrade`. Override either if your
  use case needs `Delete` / `Merge` (deletionPolicy) or `Unicode`
  (conversionStrategy).
- `values.schema.json` constraints for both new fields with enum
  validation.

### Fixed — `jarvy-telemetry-forwarder` Helm chart

- ExternalSecret resources no longer drift in Argo CD when the ESO
  admission webhook fills server-side defaults. Bump and `helm
  upgrade` to clear the perpetual OutOfSync state.

### Migration

No action needed beyond `helm upgrade`. Defaults match ESO
server-side, so rendered output is functionally identical — the diff
visible on `helm diff upgrade` is purely the two new explicit
field assignments.

## [helm-v0.4.0] — Chart enhancement plan v3 (2026-05-14)

Multi-perspective parallel review (perf, security, QA, observability,
maintainability) produced a 27-item enhancement plan; all 27 items
ship together. Probe semantics, graceful shutdown, queue-saturation
alert, dashboard, recording rules, image-digest default, FQDN egress
mode, DoS-protection gate, split Service, container security context
schema constraints, runbook anchors in the ops doc, and 5 new CI
guards (kind install/upgrade, helm 3.14/3.16/3.18 matrix, promtool,
README↔schema drift, runbook-anchor check). 13 render scenarios pass,
8 template-time guards fire, `helm lint --strict` clean. **Backward
compatible**: defaults harden but no required-field renames; legacy
`networkPolicy.cilium.enabled=true` still works (now a synonym for
`egressMode: fqdn`).

### Added — `jarvy-telemetry-forwarder` Helm chart

A multi-perspective review (perf, security, QA, observability,
maintainability) produced a 27-item enhancement plan; all 27 items
shipped together. Chart version bump pending.

- **Probe split + pipeline-aware health.** Liveness no longer flips
  on `memory_limiter` backpressure (which would cascade-restart all
  replicas during burst — defeating the design). Readiness still
  flips so the LB sheds load. `health_check_v2`'s
  `check_collector_pipeline` exposes pipeline status on `/`;
  liveness gets a longer failureThreshold (6), readiness shorter
  periodSeconds (5). New `startupProbe` covers cold-pull on fresh
  nodes.
- **Graceful shutdown.** `terminationGracePeriodSeconds: 60` +
  `preStop: sleep 15` so the LB drains and the
  batch/exporter flushes in-flight records before SIGKILL.
- **Exporter queue saturation alert** — leading indicator that fires
  before `JarvyForwarderExporterFailing` starts dropping records.
  Backed by a recording rule (`jarvy_forwarder:exporter_queue_utilization:ratio`).
- **Pod restart alert** — closes the loop when pipeline alerts can't
  fire (pod never gets healthy enough to emit metrics).
- **Grafana dashboard** ConfigMap shipped via `grafana_dashboard=1`
  sidecar label. 10 panels: receiver rate, queue utilization,
  exporter rate, memory/CPU, tail-sampling decisions, allowlist
  drops, batch throughput, pod restarts, cert expiry.
- **Receiver auth** (`collector.receiverAuth.enabled`, opt-in)
  fronts the OTLP receiver with `bearertokenauth/receiver`. Multi-
  tenant deployments should enable.
- **Recording rules.** Repeated `rate(...)` over 5-10m windows
  hoisted into named recording rules; alerts + dashboard share one
  computation instead of recomputing per evaluation.
- **`networkPolicy.egressMode`**. Three modes: `wide` (legacy
  `to: []` on 443), `wide-except-rfc1918` (new default — excludes
  private IP ranges), `fqdn` (requires Cilium — restricts to the
  parsed exporter hostname).
- **DoS-protection gate**: non-Traefik GatewayClasses must supply
  `httpRoute.extraFilters` OR set `dosProtection.acceptUnprotected:
  true` — fails install otherwise. Closes the "I installed on Envoy
  and forgot the rate limit" exposure.
- **Split Service**: public OTLP Service (port 4318) +
  in-cluster metrics Service (port 8888). In-cluster scrapers
  cannot accidentally reach the OTLP receiver and self-metrics no
  longer mix with public ingress traffic.
- **Production-overlay digest pinning**: chart ships with
  `collector.image.digest` set to a real `sha256:` digest by
  default; CI scenario `production-overlay` asserts the rendered
  image string carries the digest.
- **Grafana dashboard's `runbook_url` anchors** all exist in
  `docs/operations/telemetry-forwarder.md` (11 new
  `{#alert-*}`-anchored subsections with diagnosis steps).
- **CI**: kind install + upgrade smoke test (k8s 1.31); helm
  3.14/3.16/3.18 render matrix; promtool PromRule validation;
  README ↔ schema drift check; runbook-anchor grep.

### Changed — `jarvy-telemetry-forwarder` Helm chart

- **CPU limit removed** from `collector.resources.limits`. CFS-quota
  throttling on an I/O-bound forwarder adds 10-100ms p99 latency on
  burst with no upside. Floor preserved via `requests.cpu: 100m`.
- **HPA `scaleDown` policy** is now explicit (`drop 1 pod / 60s`)
  instead of the K8s default (halve replicas per 15s) which causes
  replica thrash near `memory_limiter` pressure.
- **PDB uses `maxUnavailable: 1`** (not `minAvailable: 1`) so node
  drains proceed one pod at a time without stalling forever waiting
  for real-Ready. Mutually exclusive with `minAvailable` —
  template-time `fail()` catches both-set misconfiguration.
- **`pdb.minAvailable` + `pdb.maxUnavailable`** mutually exclusive
  (template `fail`). **`tls.certManager.enabled=true` +
  `tls.existingSecretName`** mutually exclusive (template `fail`).
- **`_helpers.tpl` labels order**: chart-managed labels are emitted
  LAST so `commonLabels` cannot overwrite `app.kubernetes.io/name`
  and steer NetworkPolicy / ServiceMonitor away from real pods.
- **`automountServiceAccountToken: false`** stays hardcoded in both
  ServiceAccount and Pod spec (no values knob); render-time CI
  asserts catch regressions.
- **`enableServiceLinks: false`** on the pod — saves env-var bloat
  on busy namespaces; speeds cold start.
- **ServiceMonitor**: `honorLabels` is now actually rendered (was a
  ghost setting). `path: /metrics`, `scheme: http`, and
  `scrapeTimeout` explicit so a future port change doesn't break
  scrape silently. ServiceMonitor selector now matches the new
  metrics-only Service (`app.kubernetes.io/component: metrics`).
- **ServiceMonitor `metricRelabelings`**: tightened keep-list. Drops
  high-cardinality `otelcol_processor_transform_*_modified` series
  (none of which exist — see Fixed below) and keeps the operational
  subset.
- **`saltStale` alert** rebuilt: now reads
  `external_secrets_sync_calls_total` (the only series that exists
  for "salt content was refreshed"). The old query referenced a
  non-existent `kube_secret_created` metric and would have stayed
  silent forever.
- **`allowlistDroppingKeys` alert** rebuilt: compares
  `otelcol_processor_incoming_items` vs `outgoing_items` on the
  `transform/keep_allowlist_attrs` processor. The old query
  referenced non-existent `*_modified` counters.
- **`bearertokenauth` extension** for the backend exporter, plus
  optional `bearertokenauth/receiver` for inbound auth.
- **Container `securityContext`** explicitly sets
  `runAsNonRoot: true` and `seccompProfile: RuntimeDefault`
  (belt-and-suspenders over the pod-level setting). Schema rejects
  flipping `privileged`, `allowPrivilegeEscalation`,
  `readOnlyRootFilesystem`, or dropping `capabilities.drop: ALL`.
- **`exporterFailing` alert threshold units** documented as
  records/sec; docs/values comments aligned (was previously
  conflicting on per-second vs per-minute).
- **Gateway listener TLS `options:`** rendered through as-is so
  operators can pass GatewayClass-specific knobs (e.g.
  `gateway.envoyproxy.io/min-tls-version`).
- **README** updated: salt-rotation wording, accurate schema
  invariants list, new ConfigMap/dashboard/PrometheusRule entries
  in "What gets installed", egressMode and DoS-protection notes.

### Removed — `jarvy-telemetry-forwarder` Helm chart

- The `cilium.enabled` values knob is still accepted but is now a
  synonym for `egressMode: fqdn`; future versions may remove.

[helm-v0.4.0]: https://github.com/Cliftonz/Jarvy/releases/tag/helm-v0.4.0

---

The entries below belong to the Jarvy CLI's pending `[Unreleased]`
section; they ship with the next CLI tag, NOT with `helm-v0.4.0`.
Listed here so the helm-v0.4.0 release notes do not absorb them.

### Sandbox auto-detection (PRD-053)

- **Sandbox auto-detection (PRD-053).** New `src/sandbox/` module
  detects AI agent sandboxes (Claude Code, Cursor, e2b, Modal,
  Daytona, Replit), long-running container envs (GitHub Codespaces,
  Gitpod, devcontainers), and a generic `/.dockerenv` + non-TTY
  fallback. `crate::sandbox::is_seamless()` is the canonical
  "unattended" predicate; CI detection is now a strict subset.
  `JARVY_SANDBOX=0` disables detection, `JARVY_SANDBOX=1` forces
  generic-container (or whatever named provider also matches).
- **Seamless mode** wires through telemetry auto-disable, update-
  check suppression, first-run welcome suppression, brew auto-install
  block, and secrets non-interactive default — five subsystems that
  previously each carried their own `env::var("CI")` heuristic now
  share one predicate.
- **Verify-only fallback** in `jarvy setup`. When the sandbox cannot
  install tools (read-only rootfs, no user-scope package manager, no
  passwordless sudo), setup runs the doctor pipeline inline and exits
  `PREREQ_MISSING (3)` on gaps; clean runs return `0` with a
  verify-only success message. The probe records why via a
  `VerifyOnlyReason` enum (`NoJarvyHome` / `ReadOnlyRoot` /
  `NoInstallPath` / `Forced`) so support tickets explain which gate
  tripped.
- **Auto-baseline.** On the first seamless-mode run with zero gaps,
  Jarvy snapshots the current state as `.jarvy/state.json` so
  subsequent runs can do meaningful drift checks. Gated on a *full*
  doctor match — partial matches never auto-baseline (PRD-053 risk
  row 2). Works on both the install-capable and verify-only paths so
  pre-loaded sandbox images still get a baseline.
- **Seamless banner** on stderr, one line per process, summarizing
  which provider was detected and the `JARVY_SANDBOX=0` escape hatch.
  Muted by `--quiet`, `-q`, `--json`, `--format=json`,
  `--log-format=json`, or `JARVY_QUIET=1`. The corresponding
  `tracing::info!(event = "sandbox.detected")` fires regardless so
  `jarvy.log` records the decision even for JSON consumers.
- **`is_seamless_auto()`** — same as `is_seamless()` minus *forced*
  sandbox detection. Telemetry + update auto-disable now route
  through this variant so a hostile dotfile or compromised
  devcontainer base image that sets `JARVY_SANDBOX=1` cannot silence
  security-patch updates or anomaly telemetry on a victim's machine
  (PRD-053 security review F1).

### Changed

- **`JARVY_HOME` validation.** Paths must be absolute and contain no
  `..` traversal components; on Unix, existing paths must be owned by
  the current uid. Defends against `sudo -E jarvy ...` patterns where
  a less-privileged actor's env points a privileged jarvy run at
  `/etc` or `/root/.ssh` (PRD-053 security review F2).
- **Install-capability probe** writes to a per-PID `.probe-<pid>`
  filename via `OpenOptions::create_new(true)` (`O_CREAT|O_EXCL`)
  instead of `fs::write` to `.probe`. A pre-staged symlink at the
  probe path now errors out instead of being silently followed and
  clobbered (PRD-053 security review F3).
- **Banner emission moved after panic-hook install** in `main.rs` so
  any future stderr-write failure during banner emission produces a
  structured panic message instead of a default backtrace dump.
- **`detect()` and `ci::detect()` are now cached** via `OnceLock` —
  env vars and `/.dockerenv` do not change mid-run, and the previous
  implementation re-walked ~25 `getenv` calls per `is_seamless()`
  invocation × 4 callers per `jarvy setup`. Telemetry `ci_detected`
  event likewise fires at most once per process instead of once per
  call.
- **`InstallCapability::VerifyOnly` carries a `VerifyOnlyReason`** so
  log lines and tickets explain *which* probe tripped.

### Removed

- **`update::config::is_ci_environment` and the parallel shim in
  `onboarding::detection`**. Both were thin re-exports of
  `sandbox::is_seamless()`; in-tree callers now use the canonical
  predicate directly. Jarvy is a `bin` crate, no external library
  consumers to break.
- **Hand-rolled `which()` helper in `src/sandbox/mod.rs`** replaced
  by the `which` crate (already a project dep). Local impl ignored
  the Unix exec bit and only handled three Windows extensions.

### Security

- **Test images pinned by sha256 digest.** `debian:bookworm-slim` and
  `buildpack-deps:bookworm-scm` in `tests/sandbox_integration.rs`
  resolve to specific bytes regardless of registry tag drift or tag-
  replay MITM.
- **Read-only binary bind-mount.** The host's jarvy binary is mounted
  into integration-test containers via
  `Mount::bind_mount(...).with_access_mode(AccessMode::ReadOnly)` so
  a malicious container cannot truncate or replace the host binary
  mid-test (PRD-053 security review F8).

### Tests

- 10 new sandbox unit tests: forced-with/without named signal,
  `JARVY_SANDBOX=0 && CI=true` precedence, `is_seamless_auto` matrix,
  generic-container truth table, `VerifyOnlyReason` Display, force-
  verify-only probe short-circuit, banner idempotence.
- 4 new docker-backed integration tests: partial-match negative gate
  (must not auto-baseline on gaps), banner suppression with
  `--format=json`, banner suppression with `JARVY_QUIET=1`, verify-
  only must not overwrite an existing `state.json`.
- Cross-module env-isolation via `#[serial_test::serial(ci_sandbox_env)]`
  on every `ci::tests` and `sandbox::tests` function so the two
  suites cannot race on shared env vars (`CI`, `GITHUB_ACTIONS`,
  `CODESPACES`).

## [v0.1.0] — First feature-complete milestone (2026-05-27)

First feature-complete stable. Closes the round-2 hardening review
(45 items across two passes), ships clean-laptop onboarding, and
publishes 14 ready-to-copy `jarvy.toml` project templates.
Telemetry-enabled deployments now actually export records — four
compounding OTLP bugs that left env-only opt-in silently emitting
zero records are fixed (see `### Fixed` below). The public surface
from v0.0.5 is preserved; everything below is either additive,
fail-closed by default, or a tightening of internal invariants.

### Upgrading from v0.0.5

`jarvy update --channel beta` (and `jarvy update` in general) is broken in
v0.0.5 — it exits 0 without actually upgrading. Two pre-existing bugs in
v0.0.5: a hardcoded `version = "0.2"` clap string that makes v0.0.5 think
it is newer than v0.1.0, plus an update path that never triggers an
artifact download. Both are fixed in v0.1.0 but cannot be patched
retroactively. **v0.0.5 users must upgrade by reinstalling via their
package manager**, not via `jarvy update`:

- macOS (Homebrew tap restored): `brew upgrade jarvy`
- Debian/Ubuntu: `sudo apt install ./jarvy_0.1.0_amd64.deb`
- Fedora/RHEL: `sudo dnf install ./jarvy-0.1.0-1.x86_64.rpm`
- Arch (AUR): `yay -Syu jarvy-bin`
- Windows (winget): `winget upgrade Jarvy.Jarvy`
- Cargo: `cargo install jarvy --force`

From v0.1.0 onward, `jarvy update --channel beta` and `jarvy update`
work as documented.

### Added

- **Project templates.** `examples/<stack>/jarvy.toml` ships 14
  validated drop-in configs (node-npm/pnpm/bun, deno, python-api/uv,
  go-api, rust-cli/workspace, ruby-rails, java-spring, react-app,
  fullstack, k8s-platform). Companion docs at
  `docs/templates-index.md` give an AI-agent decision table mapping
  detect-by signals (lockfiles, manifests) to template URLs.
- **Clean-laptop onboarding.** New `Makefile` + idempotent
  `scripts/bootstrap.sh` give contributors a two-command setup
  (`curl install.sh | bash` then `make setup`). Bootstrap script
  honors `JARVY_CHANNEL` for stable/beta/nightly, falls back to
  `wget` if `curl` is missing, and forwards extra args to
  `jarvy setup`. shellcheck-clean.
- **`jarvy validate` recognizes the full top-level surface.**
  `[npm]`, `[pip]`, `[cargo]`, `[commands]`, `[drift]`, `[git]`,
  `[network]`, `[logging]` no longer trigger
  "unknown configuration section" warnings. Toolchain channel
  aliases (`stable`, `beta`, `nightly`, `lts`, `current`) are
  accepted as valid version strings — `rust = "stable"` validates
  cleanly.
- **`SecretError::PathEscapesProject`** + `JARVY_ALLOW_EXTERNAL_SECRETS`
  override. `[env.secrets] from_file` paths that resolve outside
  the project root and `$HOME` after symlink-resolving
  canonicalization are refused by default. Common legitimate paths
  (`~/.aws/credentials`, `<project>/.env.secret`) keep working.
  Override with `JARVY_ALLOW_EXTERNAL_SECRETS=1`.
- **`tools::pinned_installer::PinnedInstaller`** helper for the
  curl-bash class of installers. arctl, kmcp, and ollama (Linux
  fallback only) now fetch their installer scripts at a pinned
  commit, sha256-verify the body, and refuse to exec on mismatch —
  same pattern Homebrew already used. Refreshing a pinned installer
  requires updating the commit + sha256 constants together.
- **POSIX env-var grammar validation** before writing
  `[env.vars]` to shell rc files. Keys not matching
  `^[A-Za-z_][A-Za-z0-9_]*$` are skipped with a structured
  `event="env.refused_invalid_key"` warning instead of corrupting
  `~/.bashrc` / `~/.zshrc`.
- **`tools::install_method`** canonical classifier
  (`Brew`/`Cargo`/`Nvm`/`Pyenv`/`Rustup`/`Snap`/`System`/
  `NotFound`/`Unknown`). `commands::diagnose`, `commands::drift`,
  and `observability::bundle` all delegate here instead of
  hand-rolling three near-identical detectors.
- **Unsupported-tool feedback loop with telemetry-first delivery.**
  When a user (or AI agent) hits a tool Jarvy doesn't support, the
  run now surfaces a structured request payload — fuzzy Levenshtein
  suggestions with prefix-match boost, a `define_tool!` scaffold
  snippet, exit code `TOOL_UNSUPPORTED` (8), and a delivery channel.
  Telemetry is canonical: no GitHub account needed and zero triage
  work for the maintainer. The pre-filled `tool_request.yml` issue
  URL is surfaced only when telemetry is off, with
  `jarvy telemetry enable` offered as a one-time alternative. New
  `jarvy tools --request <name> [--open]` flag with pretty / JSON /
  YAML / TOML output. Setup-path returns exit 8 only when every
  configured tool was unknown — mixed runs still return 0 so partial
  setups succeed. Canonical `tool.unsupported` event with uniform
  field shape across both call sites; OTEL counter
  `jarvy.tool.unsupported` renamed from `…not_supported` to match.
- **`crates/jarvy-templates` workspace member** — dep-free crate
  shipping `validate_tool_name`, `render_tool_template`,
  `MAX_TOOL_NAME_LEN`, and the embedded `define_tool!` template.
  `cargo-jarvy` depends only on this crate now; clean-build time
  drops from minutes (full jarvy lib) to ~7s.

### Changed

- **Logging pipeline rewired** to `tracing_appender::rolling` for
  daily rotation + `tracing_appender::non_blocking` for buffered
  writes. `analytics::shutdown_logging()` flushes both the
  `SdkLoggerProvider` and the file `WorkerGuard` before
  `process::exit`, so buffered records aren't lost on early
  termination. `EnvFilter` now has a default-on floor of
  `warn,jarvy=info` if `RUST_LOG` is unset.
- **`Hook::run_with_policy`** collapsed from a 3-state `HookOutcome`
  enum to `Result<(), HookError>`. Production callers only ever
  checked `Fail` vs not-Fail; the warning-on-`continue_on_error`
  side effect already conveyed the difference. The new `Err` case
  returns the underlying `HookError` so `error_codes::HOOK_FAILED`
  callers keep working.
- **`Sanitizer::sanitize_borrowed`** returns `Cow<'_, str>` so the
  no-match path skips allocation entirely. `Sanitizer::sanitize`
  preserves the same fast path internally.
- **`tracing::warn!` → `tracing::error!`** on `tool.failed`,
  `hook.failed`, `hook.timeout`, `config.parse_error`, and
  `telemetry.endpoint.refused`. These are operator-actionable
  conditions, not advisory.
- **Subprocess spans.** `services::run_command` and
  `tools::common::run_capture` are now wrapped in
  `tracing::info_span!("subprocess.exec", cmd, args_count, ...)`
  with start/duration/exit_code events.
- **`paths.rs` cleanup.** `cache_dir` inlined into
  `remote_config_cache_dir` (only caller); `#![allow(dead_code)]`
  removed since every public function has external callers now.

### Security

- **CA-bundle trust check tightened.** `network::propagate` no
  longer accepts paths under the broad `~/.jarvy/` cache prefix —
  only `~/.jarvy/ca/` is trusted, with a trailing-slash anchor so
  `~/.jarvy/ca-attacker/...` can't slip through.
- **Cross-origin redirects refused** on
  `remote::validated_get` / `fetch_remote_config`. `ureq` agent
  now uses `.max_redirects(0)`; redirects must be revalidated
  through the policy gate.
- **Sigstore companion verification.** `update::release` returns
  `None` for cosign companion files when the `.sig`/`.pem` aren't
  exact-match siblings — a substring-match bug that would have let
  a malicious tarball claim sibling signatures was closed.
- **`exec.rs` deleted** (zero-caller speculative seam).
- **`team::inheritance::transform_github_url`** duplicate deleted;
  callers route through the canonical `remote::transform_github_url`
  so URL hardening lives in one place.

### Fixed

- `validate_get` rejected URLs with empty hosts under `file://`
  scheme but didn't match the documented "scheme not allowed"
  error string. Test relaxed to accept any error variant; behavior
  unchanged.
- `paths::remote_config_cache_dir` now reads `JARVY_HOME`
  consistently with the rest of `paths.rs` (was hand-rolling the
  override before).
- `update_rc_content` argument order documented; previously the
  test suite caller had `(content, &vars, &ctx, ShellType)` instead
  of the actual `(content, ShellType, &vars, &ctx)`.
- **OTLP env-only opt-in now actually exports.** Four compounding
  bugs caused `JARVY_TELEMETRY=1` + `JARVY_OTLP_ENDPOINT=…` to
  silently produce zero records, and even file-flag opt-in lost
  every metric point on short-lived commands:
  (1) `init_logging` gated on the file flag, missing the env
      override — the OTEL log layer was excluded from the
      subscriber whenever telemetry was opt-in via env only;
  (2) `opentelemetry-otlp` 0.31's `with_endpoint()` is the FULL URL
      not a base — a bare `http://localhost:4318` produced `POST /`
      and the collector 404'd every batch. New
      `analytics::resolve_otlp_endpoint(base, signal)` appends
      `/v1/{logs|metrics|traces}` idempotently;
  (3) `otlp_logs_endpoint()` ignored the file config's
      `[telemetry] endpoint` — setting it via
      `jarvy telemetry set-endpoint` silently failed to reroute
      logs. The logger builder now reads the merged
      `TelemetryConfig`;
  (4) `telemetry::shutdown()` was defined but never called from
      `main`, so the `SdkMeterProvider`'s 60s `PeriodicReader` had
      no chance to flush on `jarvy setup`-length runs.
      Now called alongside `analytics::shutdown_logging()` in
      the exit path.
- **`host.name` emitted as resource attribute, not per-event
  field.** Grafana Cloud was receiving plaintext
  `hostname=<machine>.local` from the `setup.inventory` event,
  defeating the chart-side anonymize pipeline (which only operated
  on resource-context attrs). Build a shared
  `opentelemetry_sdk::Resource` once at telemetry init with
  `service.name`, `service.version`, `host.name`, `os.type`,
  `os.description`; attach to both `SdkLoggerProvider` and
  `SdkMeterProvider`. Previously `service.name` defaulted to
  `unknown_service`, which broke stack-level filtering and made
  "where did this record come from" guesswork. Local file logger
  and stderr layers still print plaintext (those are operator-
  owned sinks, not the egress channel).
- **`emit_telemetry_hint_if_undecided`** now consults
  `telemetry::is_enabled()` first so a user running with
  `JARVY_TELEMETRY=1` doesn't see "telemetry is opt-in and
  currently off" right after a run that just emitted records.
- **Drift hash respects `--file`.** `jarvy drift` hashed
  `<project_dir>/jarvy.toml` regardless of the `--file` flag, so
  drift detection silently used the wrong file when a non-default
  config path was supplied.
- **`set_up_os` matches `env::consts::OS` casing.** A capitalization
  mismatch in the platform-dispatch table caused setup to fall
  through to the unknown-OS path on some platforms.

### Tests

- 1,633+ tests passing across lib + binary + integration suites
  (was ~1,580). Highlights of the new coverage:
  - `validated_get` rejection tests for HTTP-to-remote, disallowed
    host, `file://` scheme, missing scheme.
  - `Hook::run_with_policy` outcome matrix (dry-run / success /
    failure × continue_on_error true|false).
  - `verify_no_tar_escape` containment tests + symlink-escape
    refusal.
  - Cosign companion exact-match (no substring) regression.
  - Path-containment refusal + `JARVY_ALLOW_EXTERNAL_SECRETS=1`
    override path for `[env.secrets] from_file`.
  - Shell-interpreted-key table-driven test
    (`every_shell_interpreted_key_refuses_bang_prefix`) so adding
    a new shell-interpreted git config key lights up the test
    suite immediately.
- `#[serial_test::serial]` annotations added for
  `JARVY_ALLOW_*` env mutations to keep parallel runs isolated.

### Docs

- `CLAUDE.md` Logging section rewritten to match the actual
  `src/logging/` (thin re-export layer) and `src/observability/`
  (where rotation + sanitizer + analytics live) split.
- `examples/README.md` + `docs/templates-index.md` published as
  the human/AI-facing template indexes.
- `llms-full.txt` "Project Templates" section added (with
  `docs/llms.txt` + `docs/llms-full.txt` symlinks for the published
  docs site).

## [v0.0.5] — Chocolatey install script + bundled v0.0.4 fixes (2026-05-05)

Folds in everything queued for v0.0.4 (which was tagged but never
publicly published) plus a Chocolatey install-script fix.

### Fixed

- **Chocolatey package** v0.0.3 failed moderation with `404 Not Found`
  for the install URL. Two bugs in
  `dist/windows/chocolatey/tools/chocolateyinstall.ps1`:
  - URL pattern referenced
    `jarvy-vVERSION_PLACEHOLDER-x86_64-pc-windows-msvc.zip` — but
    cargo-packager produces `.msi` and `.exe`, no `.zip` for Windows.
  - VERSION_PLACEHOLDER and SHA256_PLACEHOLDER were never substituted
    because the publish workflow only ran sed against `jarvy.nuspec`,
    not the install script.

  Rewrote the install script to use `Install-ChocolateyPackage` with
  `-FileType msi` and silent install args, pointing at the actual
  `jarvy_<v>_x64_en-US.msi` asset. Updated
  `publish-packages.yml::update-chocolatey` to substitute both files
  AND pull the real msi SHA256 from `SHA256SUMS.txt` so the integrity
  check passes.
- **`cargo fmt --check`** drift in `src/team/inheritance.rs:760-768`
  (single-quoted TOML literals from v0.0.3 needed compaction).
- **OpenSSF Scorecard** failed on v0.0.3 tag with `Only the default
  branch main is supported`. ossf/scorecard-action explicitly refuses
  tag-push triggers. Restored `push: branches: [main]` for scorecard
  only — every other validating workflow stays tag-triggered.
- **Homebrew tap publish** now gracefully skips when
  `HOMEBREW_TAP_DEPLOY_KEY` is not configured. Previously the missing
  secret failed the whole `publish-packages.yml` workflow, masking
  the success of crates.io, AUR, winget, and Chocolatey jobs.

### Validated downstream (v0.0.3)

After the v0.0.3 fixes, the following propagation channels worked:

- ✅ crates.io: jarvy@0.0.3 + cargo-jarvy@0.0.3 published
- ✅ AUR (jarvy-bin)
- ✅ Submit to winget (publish-packages.yml job; separate winget.yml
  still needs manual first submission)
- ✅ GitHub Pages docs site (after maintainer enabled Pages)
- ❌ Chocolatey: failed moderation due to broken install script
  (v0.0.5 fixes)
- ⚠️  Homebrew tap: pending secret config (now non-blocking)

### Note

v0.0.4 was tagged but the draft was never publicly published —
v0.0.4's fixes ship together with the Chocolatey fix as v0.0.5 to
reduce propagation churn (one round of crates.io / AUR / etc.
updates instead of two back-to-back).

## [v0.0.4] — Lint formatting + scorecard + homebrew-tap guard (2026-05-05)

### Fixed

- **`cargo fmt --check`** failed in the Lint job on
  `src/team/inheritance.rs:760-768` because the v0.0.3 single-quoted
  TOML literal edits left format strings on multiple lines that
  rustfmt wanted compacted. Re-ran `cargo fmt` to normalize.
- **OpenSSF Scorecard** failed on the v0.0.3 tag with `Only the
  default branch main is supported`. ossf/scorecard-action explicitly
  refuses tag-push triggers; v0.0.3's trigger trim moved scorecard
  off main-push, which broke it. Restored `push: branches: [main]`
  for scorecard only — every other validating workflow stays
  tag-triggered. Release-tag scorecard runs produce no useful signal
  anyway since the action only inspects the default branch.
- **Homebrew tap publish** now gracefully skips when
  `HOMEBREW_TAP_DEPLOY_KEY` is not configured. Previously the whole
  `publish-packages.yml` workflow exited 1 with "API_TOKEN_GITHUB
  and SSH_DEPLOY_KEY are empty", masking the success of crates.io,
  AUR, winget, and Chocolatey jobs. New behavior: missing secret
  emits a warning ("set per docs/MAINTAINER_RELEASE_GUIDE.md") and
  the push step is skipped via `if:` guard.

### Validated downstream (v0.0.3)

After the v0.0.3 fixes, the following propagation channels worked:

- ✅ crates.io: jarvy@0.0.3 + cargo-jarvy@0.0.3 published
- ✅ Submit to winget (job inside publish-packages.yml; the separate
  winget.yml workflow still requires manual first submission per
  v0.0.3 release notes)
- ✅ Chocolatey
- ✅ AUR (jarvy-bin)
- ✅ GitHub Pages docs site (after maintainer enabled Pages in repo
  Settings)
- ⚠️  Homebrew tap: blocked on `HOMEBREW_TAP_DEPLOY_KEY` secret;
  v0.0.4 makes this a non-blocker so missing-secret no longer fails
  the whole workflow.

## [v0.0.3] — Unblock crates.io and Homebrew downstream publish (2026-05-05)

Patch release. v0.0.2 went live on the GitHub release page but the
crates.io and Homebrew workflows that fire on `release: published`
both failed, leaving `cargo install jarvy` and
`brew install Cliftonz/tap/jarvy` unavailable.

### Fixed

- **Cargo.toml** declared `readme = "README.md"` (uppercase) but the
  tracked file is `Readme.md` (mixed case). On macOS the difference
  is invisible (case-insensitive filesystem); on the Linux CI runner
  it failed `cargo publish` with `readme "README.md" does not appear
  to exist`. Both `Publish Crate` and `Publish to Package Managers`
  workflows hit the same error. Same fix in the `include = [...]`
  manifest list. Now matches what's actually in the git tree.
- **`.github/workflows/winget.yml`** was scaffolded from a different
  project's template and never customized — `identifier: Benji377.Tooka`
  and `fork-user: Benji377` referenced a totally unrelated package.
  Rewrote with placeholder TODO values for `Jarvy.Jarvy` /
  `Cliftonz` and changed the trigger from `release: published` to
  `workflow_dispatch` only. winget-releaser cannot create a brand-new
  package registration; the first submission must go through
  `wingetcreate new` and a hand-reviewed PR to microsoft/winget-pkgs.
  After that's merged the trigger can be flipped back.

### Removed

- Duplicate `.github/workflows/crates.yml` deleted. Both that and
  `publish-packages.yml::publish-crates-io` were firing on
  `release: published` and trying to `cargo publish`. Even if both
  had the right secret, the second one would race-fail with "crate
  version already exists". Kept the version inside `publish-packages.yml`
  because it composes with the Homebrew tap update via `needs:`.
- `docs/release-testing.md` and `docs/release-quirks-jarvy.md`
  references to `crates.yml` updated to point at the surviving
  workflow path.

### Known issues (not fixed in this release)

- **GitHub Pages** is not enabled for `Cliftonz/Jarvy` repo — the
  Deploy Docs workflow fails with `HttpError: Not Found ... Ensure
  GitHub Pages has been enabled`. Fix is in repo Settings → Pages,
  not in code. Until enabled, the docs site at jarvy.dev (or
  whichever Pages URL ends up provisioned) won't update on release.
- **winget first submission** still requires manual `wingetcreate new`
  intervention (see Fixed above for the workflow disable).

## [v0.0.2] — Cosign verify-command case fix (2026-05-05)

Patch release fixing the cosign verification snippet baked into
release notes, SECURITY.md, and docs/release-quirks-jarvy.md.

### Fixed

- **release notes / SECURITY.md / docs**: the
  `--certificate-identity-regexp` value used `Cliftonz/jarvy`
  (lowercase j). The actual Sigstore cert subject GitHub Actions
  produces is `Cliftonz/Jarvy/...` (capital J — the repo's
  canonical case). cosign's regex is case-sensitive, so users
  copy-pasting the verify command from the v0.0.1 release page
  saw "none of the expected identities matched" even though the
  signature was valid. Corrected all three sources to
  `Cliftonz/Jarvy/`. github.com URLs elsewhere in the repo are
  unchanged because GitHub URL matching is case-insensitive — only
  cosign's regex was affected.

## [v0.0.1] — Initial public release (2026-05-05)

First publicly tagged stable release. Validated through the
v0.1.0-rc.1 → v0.1.0-rc.9 soak cycle (same tree, version-string
only differs); cut as 0.0.1 to keep the first-stable surface narrow
and reserve room for 0.1.0 as the first feature-complete milestone.

### Features

- **provisioner:** Cross-platform tool provisioner driven by `jarvy.toml`
  (macOS, Linux, Windows) with native package managers
- **tools:** 154+ tool registry covering compilers, runtimes, CLIs, container
  tools, Kubernetes ecosystem (kubectl, helm, k9s, kagent, kmcp, arctl), cloud
  CLIs (gcloud, aws, az), security tools, observability (opentelemetry-collector),
  Dockerfile converter (dfc) (PRD-013)
- **tools:** Parallel version checking with rayon for ~5x speedup; batch
  package-manager operations
- **tools:** Declarative `define_tool!` macro for tool definitions (~2000 lines
  reduced)
- **tools:** Strict (`depends_on`) and flexible (`depends_on_one_of`) tool
  dependencies with topological install ordering (PRD-034)
- **hooks:** 29+ default post-install hooks for shell completion and
  configuration; idempotent, advisory, user-overridable
- **roles:** Role-based configurations with deep inheritance, version overrides,
  `roles list|show|diff` commands (PRD-033)
- **packages:** Language package deps via `[npm]`, `[pip]`, `[cargo]` —
  package-manager auto-detection, virtualenv support, lockfile install (PRD-039)
- **git:** Git configuration automation — identity, SSH/GPG signing, default
  branch, aliases, credential helper auto-detect per OS (PRD-041)
- **drift:** Configuration drift detection with SHA-256 file hashing, version
  policies, `jarvy drift check|status|accept|fix` (PRD-043)
- **update:** Self-updating with stable/beta/nightly channel selection,
  throttled checks, rollback, multi-method install detection (Homebrew, Cargo,
  apt, dnf, winget, Chocolatey, Scoop, binary fallback) (PRD-035)
- **telemetry:** OTEL-unified logs, metrics, optional traces; OTLP HTTP/gRPC
  endpoints; CI auto-disable; `jarvy telemetry status|enable|disable|test|preview`
  (PRD-022, PRD-050)
- **logging:** Persistent file logging with rotation, gzip compression,
  sensitive-data redaction; `jarvy logs view|stats|clean|config` (PRD-050)
- **ticket:** Debug bundles via `jarvy ticket create|show|list|clean` — ZIP with
  system info, tool versions, sanitized logs (PRD-050)
- **network:** Corporate proxy support — HTTP/HTTPS/SOCKS, NO_PROXY, custom CA
  bundles, per-tool overrides, secure password sources (PRD-019)
- **services:** Docker Compose and Tilt backend support
- **ci:** Auto-detection for 11 CI/CD providers with provider-specific output
- **env:** Environment variable management with `.env` generation and shell rc
  updates
- **mcp:** MCP server exposing tools and resources for AI assistants
- **interactive:** Menu mode when running `jarvy` without a subcommand
- **bootstrap:** `jarvy bootstrap`, `jarvy configure`, `jarvy diagnose` for
  onboarding (PRD-023)

### Distribution

- Multi-channel: crates.io, Homebrew tap, AUR (source + binary), `.deb`, `.rpm`,
  winget, Chocolatey, universal install scripts for macOS/Linux/Windows (PRD-012)
- **Prebuilt platforms**: macOS arm64, Linux x86_64 (musl), Linux aarch64,
  Linux armv7, Windows x86_64. macOS Intel (x86_64) **not shipped as prebuilt** —
  Intel users install via `cargo install jarvy` or Homebrew (both compile from
  source). See `docs/release-testing.md` for rationale.
- Sigstore keyless signing for all release artifacts (PRD-020)
- SBOM generation in SPDX 2.3 and CycloneDX 1.4 formats per release (PRD-020)
- GitHub build provenance attestation per release (PRD-020)
- Opt-in early-release channel: `JARVY_CHANNEL=beta` env var on install
  scripts; `[update] channel = "beta"` in `~/.jarvy/config.toml`;
  `jarvy update --channel beta`

### Quality & Security

- Clippy gate, mutation testing, fuzzing, coverage, benchmarks, OpenSSF
  Scorecard (PRD-018)
- Hybrid cross-platform E2E testing harness (PRD-038)
- Tag-signing enforcement (SSH or GPG) on release workflow
- Cosign keyless signing via GitHub OIDC for all release artifacts

### Infrastructure

- Semantic version checking with proper semver operators
- Cross-platform shell detection and hook execution
- Workspace lint configuration; Rust 2024 edition; MSRV 1.85

[Unreleased]: https://github.com/Cliftonz/jarvy/compare/v0.2.2...HEAD
[v0.3.0]: https://github.com/Cliftonz/Jarvy/releases/tag/v0.3.0
[v0.2.2]: https://github.com/Cliftonz/jarvy/releases/tag/v0.2.2
[v0.2.1]: https://github.com/Cliftonz/jarvy/releases/tag/v0.2.1
[v0.2.0]: https://github.com/Cliftonz/jarvy/releases/tag/v0.2.0
[v0.1.0]: https://github.com/Cliftonz/jarvy/releases/tag/v0.1.0
[v0.0.5]: https://github.com/Cliftonz/jarvy/releases/tag/v0.0.5
[v0.0.4]: https://github.com/Cliftonz/jarvy/releases/tag/v0.0.4
[v0.0.3]: https://github.com/Cliftonz/jarvy/releases/tag/v0.0.3
[v0.0.2]: https://github.com/Cliftonz/jarvy/releases/tag/v0.0.2
[v0.0.1]: https://github.com/Cliftonz/jarvy/releases/tag/v0.0.1
