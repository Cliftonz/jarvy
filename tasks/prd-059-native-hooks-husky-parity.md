# PRD-059 — Native Git Hooks: Husky-Parity for Any Repo

- **Status:** proposed
- **Created:** 2026-08-03
- **Priority:** medium
- **Estimated:** 5 days
- **Depends on:** PRD-048 (git hooks framework integration — shipped)

## Problem

Users coming from the Node ecosystem love the Husky workflow: hook
scripts are files committed to the repo, they install themselves on
clone, and adding a hook is one command. Outside Node repos there is no
equivalent — Husky requires `package.json` + npm, `pre-commit` requires
Python and a YAML DSL, and lefthook is another binary + another config
dialect.

Jarvy already has 80% of the machinery (PRD-048):

- `[git_hooks.native]` writes inline TOML hook bodies into
  `.git/hooks/<stage>` with a managed-by-jarvy marker and a
  refuse-to-clobber guard for hand-rolled hooks.
- `auto_install = true` runs during `jarvy setup` — the equivalent of
  Husky's `prepare` script, but language-agnostic.
- `jarvy hooks {install,update,list,run,status}` is the CLI surface.
- The `allow_remote` trust gate stops remote configs from landing
  arbitrary hooks.

What's missing is the *experience* that made Husky sticky:

1. **Hooks as repo files, not inline TOML strings.** `.husky/pre-commit`
   is a real shell file — editable, shellcheck-able, diffable, with
   syntax highlighting. `hooks.pre-commit = """..."""` inside
   `jarvy.toml` is none of those, and multi-line TOML strings get
   unreadable past ~10 lines.
2. **Zero-drift install.** Husky v9 points `core.hooksPath` at a
   directory tracked by git, so editing a hook needs no reinstall step.
   Jarvy's native handler copies bodies into `.git/hooks/`, which
   drifts the moment someone edits `jarvy.toml` and forgets to re-run
   `jarvy hooks install`.
3. **One-command authoring.** Husky: `echo "npm test" > .husky/pre-commit`.
   Jarvy today: open `jarvy.toml`, find the right table, write a TOML
   multi-line string with correct escaping.

## Goal

Make `jarvy` the shortest path to committed, self-installing git hooks
in **any** repo — Rust, Go, Terraform, k8s — with no language runtime
and no extra binary beyond jarvy itself.

## Non-Goals

- Replacing the `pre-commit` / husky / lefthook wrappers (they stay;
  detection order unchanged).
- A lint-staged clone with full glob-filtering pipelines (a minimal
  staged-files variable is in scope; the pipeline DSL is not).
- Hook execution on Windows outside git's own `sh.exe` environment
  (git-for-windows runs every hook through its bundled sh; we target
  that, not PowerShell hook bodies).

## Design

### 1. File-based hooks: `.jarvy/hooks/`

New optional layout, coexisting with inline bodies:

```
.jarvy/hooks/
  pre-commit          # plain shell file, committed to the repo
  commit-msg
  pre-push
```

```toml
[git_hooks]
framework = "native"

[git_hooks.native]
dir = ".jarvy/hooks"     # NEW — file-based mode
# hooks.pre-commit = "…" # existing inline mode still works
```

Rules:

- `dir` and inline `hooks.*` may coexist; a stage defined in both is a
  config error (`HookError::Config`) — no silent precedence.
- Files must be named exactly a known git stage (same `is_stage_name`
  validation as inline keys). Unknown filenames are a config error, not
  skipped — a typo'd `pre-comit` failing loudly beats a hook that never
  fires.
- Path traversal: `dir` must resolve inside the project root (reuse the
  `resolve_within_workspace` containment pattern from
  `mcp/extended_tools`).

### 2. Install modes: `copy` (default) vs `hooks-path`

```toml
[git_hooks.native]
dir = ".jarvy/hooks"
install = "hooks-path"   # NEW — or "copy" (default, current behavior)
```

- **copy** — current behavior: write into `.git/hooks/<stage>` with the
  jarvy marker. Works with other tools that also write `.git/hooks`.
- **hooks-path** — Husky-v9 style: `git config core.hooksPath .jarvy/hooks`.
  Zero drift (edit the file, hook is live), and the hooks are what's in
  git — no reinstall step ever.
  - Refusal: if `core.hooksPath` is already set to a different value
    (husky's `.husky/_`, a user's dotfiles path), refuse with a clear
    error instead of stealing it. Mirrors the marker-guard philosophy.
  - `chmod +x` every hook file at install (git requires the exec bit;
    fresh clones on some filesystems lose it).
  - Uninstall (`jarvy hooks uninstall`, new subcommand): unset
    `core.hooksPath` only if it points at our `dir`.
- `install = "hooks-path"` with inline-only hooks is a config error —
  there is no directory to point at.

### 3. Authoring CLI: `jarvy hooks add`

```bash
jarvy hooks add pre-commit "cargo fmt --check"
# → creates .jarvy/hooks/pre-commit (or appends a command line to it),
#   chmod +x, prints a reminder to commit the file
# → if [git_hooks] is absent from jarvy.toml, offers to write the
#   minimal block (respecting --yes for scripting)
```

- Appending to an existing hook file adds the command on a new line
  (Husky v9 semantics). `--force` recreates the file.
- Command text is written verbatim into the script — same consent model
  as `jarvy run` (explicit user-typed command = consent), but the
  Trojan-Source/control-byte sanitizer from `config.rs` applies to
  refuse NUL/bidi content.

### 4. Staged-files variable (minimal, not lint-staged)

Inside a jarvy-managed hook body, the literal token `{staged_files}` is
replaced at **install time** with a portable sh snippet:

```sh
STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACMR)
```

…and the token becomes `"$STAGED_FILES"`. That's it — no per-glob
pipelines, no automatic re-staging. Documented as a convenience, with a
pointer to `pre-commit` for anything fancier.

### 5. Skip mechanism

- `JARVY_SKIP_HOOKS=1` — every jarvy-written hook script begins with a
  stamped guard line:

  ```sh
  [ -n "$JARVY_SKIP_HOOKS" ] && exit 0
  ```

- Per-stage: `JARVY_SKIP_HOOKS=pre-push` (comma-separated list matches
  stage names; `1`/`true` = all). Guard logic is generated, ~4 lines of
  sh, identical in every hook.
- git's own `--no-verify` continues to work for commit/push stages —
  document both.

### 6. Bootstrap parity

`scripts/bootstrap.sh` + `jarvy setup` already auto-install
(`auto_install = true` default). Add one doc section to
`docs/git-hooks.md` + `docs/replace-husky.md`: "Path 4 — jarvy native
(no framework at all)", showing a Node repo dropping Husky entirely:

```bash
npm uninstall husky && rm -rf .husky
git config --unset core.hooksPath
jarvy hooks add pre-commit "npm test"
git add .jarvy/hooks jarvy.toml && git commit
```

### 7. Trust boundary (unchanged, restated)

- Remote configs (`--from <url>`) cannot auto-install any hook without
  `[git_hooks] allow_remote = true` — file-based mode changes nothing.
- `hooks-path` mode is MORE sensitive for remote configs (it redirects
  every hook stage at once): remote + `install = "hooks-path"` is
  refused even with `allow_remote = true` unless the hooks dir already
  exists in the working tree (i.e., the user cloned it knowingly).
- Hook file contents are never fetched from the network by this
  feature. Library-sourced hooks stay in the `[ai_hooks]` domain.

## Telemetry

| Event | Fields | Notes |
|---|---|---|
| `git_hooks.installed` | existing + `mode = "copy" \| "hooks-path"`, `source = "inline" \| "dir"`, `count` | extend existing event |
| `git_hooks.hooks_path_refused` | `reason = "foreign_hooks_path" \| "remote_config" \| "inline_only"` | new, warn |
| `git_hooks.hook_added` | `stage`, `created` (bool — new file vs append) | new; command text NEVER emitted |
| `git_hooks.uninstalled` | `mode` | new |

All gated by `telemetry_gate::is_enabled()` per the standing contract.

## CLI Surface Delta

| Command | Change |
|---|---|
| `jarvy hooks add <stage> <cmd>` | new |
| `jarvy hooks uninstall` | new |
| `jarvy hooks install/update/list/run/status` | learn file-based dir + hooks-path mode |

## Acceptance Criteria

1. A repo with zero Node/Python tooling gets a committed, self-installing
   `pre-commit` hook via two commands (`jarvy hooks add …`, `git commit`),
   and a fresh clone activates it with `jarvy setup` alone.
2. `install = "hooks-path"` never overwrites a foreign `core.hooksPath`.
3. Editing `.jarvy/hooks/pre-commit` in hooks-path mode changes behavior
   with no reinstall.
4. Inline `[git_hooks.native.hooks]` configs keep working byte-for-byte
   (existing tests untouched).
5. `JARVY_SKIP_HOOKS=1 git commit` bypasses jarvy-written hooks.
6. Remote-config trust matrix covered by integration tests (allow_remote
   × install mode × dir-exists).
7. Windows: hooks execute under git-for-windows sh; CI job proves it.

## Phasing

- **Phase 1 (core, ~3d):** file-based `dir`, `copy` mode support,
  stage-name validation, `jarvy hooks add`, skip guard, docs.
- **Phase 2 (~2d):** `hooks-path` mode + uninstall + refusal matrix,
  `{staged_files}`, telemetry extensions, Windows CI proof.

## Open Questions

1. Should `jarvy discover` suggest `[git_hooks]` when it sees `.husky/`
   (migration nudge) — or is that too pushy for v1?
2. `jarvy hooks add` writing into `jarvy.toml` automatically: comment
   preservation in TOML round-trip is lossy with `toml` crate — may need
   `toml_edit`. Decide before Phase 1.
3. Per-hook timeout guard (runaway hook blocks every commit)? Deferred
   unless cheap.
