---
title: "Git hooks — Jarvy"
description: "Install and manage git hooks directly from jarvy.toml — no third-party framework. Native handler supports every git hook stage, with inline / file / folder / repo source shapes."
tags:
  - guides
  - git
---

# Git hooks

`jarvy hooks` installs and manages git hooks directly from `jarvy.toml`. The intent: **every contributor gets the same commit / push / message gates without anyone running any install command by hand — and without a third-party framework in the loop.**

The **native handler** writes hook scripts straight into `.git/hooks/<stage>` and stamps a `# managed by jarvy` marker so future runs recognize their own output. It covers every git hook stage (`pre-commit`, `pre-push`, `commit-msg`, `post-checkout`, `pre-rebase`, `applypatch-msg`, …).

[Husky](https://typicode.github.io/husky/) and [Lefthook](https://github.com/evilmartians/lefthook) are still auto-detected but their handlers are stubbed — the CLI reports "framework configured but not yet supported" rather than silently no-op-ing.

> **v0.8 removed the pre-commit framework integration.** Jarvy no longer shells out to `pre-commit install` and doesn't auto-detect `.pre-commit-config.yaml`. Migrate by moving your hook scripts into `scripts/hooks/` and setting `dir = "scripts/hooks"` — see [Migrating from pre-commit](#migrating-from-pre-commit) below.

---

## Why `[git_hooks]` and not `[hooks]`

`[hooks]` is already taken by `jarvy setup` for `pre_setup` / `post_install` / `post_setup` shell scripts (PRD-003). Mixing git hooks into that block would tangle two unrelated lifecycles, so git hooks live under their own `[git_hooks]` top-level block. They can be used independently.

---

## Configuration

Minimal opt-in — jarvy auto-detects husky or lefthook if the project uses one:

```toml
[git_hooks]
```

Full shape:

```toml
[git_hooks]
enabled = true                    # default true
framework = "native"              # native | husky | lefthook (pre-commit removed in v0.8)
auto_install = true               # install during `jarvy setup` (default true)
auto_update = false               # (reserved — no-op for native handler today)
run_after_install = false         # run hooks once against the whole tree (default false)
allow_remote = false              # remote-config trust gate (default false)

# Native handler — the primary knob. Four source shapes, listed in
# precedence order (later sources override earlier for the same stage).

[git_hooks.native]
# 1. Remote git repo — clone into ~/.jarvy/git_hooks_cache/<hash>/
#    and scan for stage-named files. Great for a team-wide hooks
#    library shared across many repos.
repo = "github:acme/team-hooks"   # or https://... or git+https://...
ref  = "v1.2.0"                   # REQUIRED — SHAs & v-tags treated as pinned
subpath = "hooks"                 # optional folder within the repo

# 2. Local folder scan — any file whose name matches a git hook stage
#    gets installed. Non-stage files (README.md, .gitkeep) ignored.
dir = "scripts/hooks"

# 3. Explicit per-stage entries — inline body OR file reference.
#    Overrides both `repo` and `dir` for the same stage name.
[git_hooks.native.hooks]
commit-msg = "#!/bin/sh\ngrep -qE '^(feat|fix|chore):' \"$1\""
pre-push   = { file = "ci/pre-push.sh" }
```

### Precedence

For a given hook stage, later sources win:

`repo` → `dir` → explicit `hooks` entry.

So a team can point at a shared `repo` for the common defaults, override one hook via `dir` for the whole project, and override one more via `hooks.<stage>` for a single machine.

### Trust boundary

Same shape as `[packages] allow_remote`. A `jarvy.toml` fetched via `jarvy setup --from <url>` is refused at the hook-install gate unless `[git_hooks] allow_remote = true` is set **in the source config**. Setting `allow_remote = true` in your own local config does NOT broaden trust for files you fetch from elsewhere.

Remote-config refusal logs a `git_hooks.remote_refused` event and continues with the rest of the setup run.

---

## The `[git_hooks.native]` source shapes

### 1. Remote repo (`repo` + `ref`)

Shared team hooks library:

```
your-org/team-hooks/
├── pre-commit          # installed as .git/hooks/pre-commit
├── pre-push            # installed as .git/hooks/pre-push
├── commit-msg          # installed as .git/hooks/commit-msg
└── README.md           # ignored (not a stage name)
```

```toml
[git_hooks.native]
repo = "github:your-org/team-hooks"
ref  = "v1.2.0"
```

**URL shapes:**

- `github:owner/repo` — shorthand for `https://github.com/owner/repo.git`
- `https://host/path/repo.git` — plain HTTPS clone
- `git+https://host/path/repo.git` — accepted for parity with `library_sources`

**`ref` is required.** Unpinned URLs are refused at parse time so a publisher can't silently rev the hook body a consumer executes. SHAs (7–40 hex chars) and `v`-prefixed tags (`v1.2.0`) are treated as pinned; branch names (`main`, `develop`) trigger a `git_hooks.mutable_ref` warning because the publisher can rewrite them.

**`subpath`** narrows the scan to a subdirectory of the repo. Refused if absolute or contains `..`.

**Caching:** clones go to `~/.jarvy/git_hooks_cache/<sha256(repo+ref)>/`. Ref-hashed so a `ref` bump lands in a fresh cache dir instead of racing an in-flight fetch. Repeat installs run `git fetch + reset --hard <ref>` idempotently.

### 2. Local folder scan (`dir`)

Point at a folder in the project. Every file whose bare name matches a git hook stage gets installed; non-stage files (READMEs, .gitkeep, misspellings) are silently ignored:

```
your-repo/
├── jarvy.toml
└── scripts/
    └── hooks/
        ├── pre-commit          # executable, gets installed
        ├── pre-push            # executable, gets installed
        ├── commit-msg          # executable, gets installed
        └── README.md           # ignored
```

```toml
[git_hooks.native]
dir = "scripts/hooks"
```

### 3. File reference (`hooks.<stage> = { file = "..." }`)

Point at a specific script anywhere in the project:

```toml
[git_hooks.native.hooks]
pre-commit = { file = "ci/hooks/pre-commit.sh" }
pre-push   = { file = "ci/hooks/pre-push.sh" }
```

File paths are relative to the project root. Refused if absolute, contain `..` segments, or canonicalize outside the project (symlink escape is blocked).

### 4. Inline body (`hooks.<stage> = "..."`)

Short scripts pasted directly into `jarvy.toml`:

```toml
[git_hooks.native.hooks]
commit-msg = """
#!/bin/sh
grep -qE '^(feat|fix|chore|docs|refactor|test): ' "$1" || {
  echo "Commit message must start with feat:/fix:/chore:/docs:/refactor:/test:"
  exit 1
}
"""
```

---

## Marker + overwrite safety

Every installed hook is stamped:

```sh
#!/bin/sh
# managed by jarvy — [git_hooks.native]
# ...your hook body...
```

- **First install** with no existing `.git/hooks/<stage>`: writes the file, `chmod +x`, done.
- **Second install** when jarvy owns the file: overwrites safely (marker present).
- **Install with an unmarked hook already there:** refuses with `HookError::InstallFailed`. Move the hand-authored file aside, add the marker manually, or run `jarvy hooks uninstall` (removes only marker-bearing files).

---

## Commands

```bash
jarvy hooks install            # write scripts into .git/hooks/
jarvy hooks update             # re-run install (no separate update semantics for native)
jarvy hooks status             # framework + installed?  + hook count
jarvy hooks list               # print configured hooks (from repo + dir + hooks map)
jarvy hooks run                # run all configured hooks
jarvy hooks run --hook pre-push  # run one stage
jarvy hooks uninstall          # remove jarvy-marker-bearing files from .git/hooks/
```

`jarvy setup` auto-runs `jarvy hooks install` between the git-config phase and the AI-hooks phase, gated on `[git_hooks].auto_install`.

---

## Migrating from pre-commit

If you were using `[git_hooks.pre_commit]` (removed in v0.8) or an existing `.pre-commit-config.yaml`, the path forward depends on how much of the pre-commit ecosystem you need:

1. **You only used pre-commit for a small set of hooks specific to your project.** Move your hook scripts into `scripts/hooks/` (or wherever you like) and set:

    ```toml
    [git_hooks.native]
    dir = "scripts/hooks"
    ```

2. **You share hooks across many repos on your team.** Publish them as a git repo, tag a version, and point at it:

    ```toml
    [git_hooks.native]
    repo = "github:your-org/team-hooks"
    ref  = "v1.0.0"
    ```

3. **You depended on pre-commit's third-party hook ecosystem** (e.g. hooks from `pre-commit/pre-commit-hooks`). Two options: keep running the `pre-commit` CLI outside of jarvy (jarvy still installs the tool via `[provisioner]`, it just no longer wires it into your git hooks), or wrap the tools those hooks call inside native scripts. For most cases (formatters like `black` / `prettier` / `rustfmt`, linters like `eslint` / `ruff`), a five-line native `pre-commit` script that calls those tools directly is simpler than the pre-commit YAML that wraps them.

`.pre-commit-config.yaml` is no longer auto-detected — nothing happens if you leave it in the tree. Delete it when you've completed the migration.

---

## Status output

```
$ jarvy hooks status
Git Hooks Status
================
Framework:    native
Installed:    yes
Hook count:   3
```

Hook count reflects everything the current config resolves to (repo + dir + hooks map, deduplicated by stage). Works even when `.git/hooks/` is empty — jarvy walks the config, not the filesystem.

---

## CI

In CI, `jarvy hooks install` is usually unnecessary — CI runs the same commands the hooks would run, directly and unconditionally, rather than installing hooks into a `.git/hooks/` directory that's discarded with the runner.

The git-hooks phase still runs in CI by default; opt out with:

```toml
[git_hooks]
auto_install = false             # in jarvy.toml
```

Or per-run:

```bash
jarvy setup --no-hooks           # skips ALL setup hooks AND git hooks
```

---

## Troubleshooting

- **`[git_hooks.native] repo `github:...` is set but `ref` is missing (required)`** — `ref` is mandatory for `repo` sources. Pin a tag or SHA.
- **`git_hooks.mutable_ref` warning event** — you pinned to a branch name. The clone still works but the publisher can rev it silently. Prefer a tag or SHA in production configs.
- **`hooks repo ... URL must start with 'github:', 'https://', or 'git+https://'`** — `git@...:` SSH URLs aren't supported (auth surface is intentionally narrow); use `https://` with a PAT or GitHub Actions token if the repo is private.
- **`refusing to overwrite a hand-rolled hook`** — an existing `.git/hooks/<stage>` predates jarvy's management. Move it aside (backup) or add `# managed by jarvy — [git_hooks.native]` on line 2 to opt into overwriting.
- **`not inside a git repository`** — git hooks need `.git/` to live. Run `git init` first.
- **`framework 'husky' is configured but not yet supported`** — only native ships today. File an issue if you need husky / lefthook prioritized.

---

## What's next

- Husky framework support (npm / yarn / pnpm workflows)
- Lefthook framework support (Go / Ruby / Rust workflows that prefer it)
- `jarvy hooks doctor` — diff configured hooks vs. `.git/hooks/` state

Track progress under `prd/048-pre-commit-hook-installation.md`.

---

## Related

- [Configuration reference](configuration.md) — full `[git_hooks]` schema
- [Hooks guide](hooks.md) — `jarvy setup` lifecycle hooks (NOT git hooks)
