# Replacing Husky with Jarvy

If your repo uses [Husky](https://typicode.github.io/husky/) today, you have two paths forward — depending on whether you want to keep npm in the hook loop.

## TL;DR

| Path | Husky stays | New dependency | When to pick it |
|---|---|---|---|
| **Wrap Husky (zero migration)** | Yes | None — Jarvy just installs Husky for you | You like Husky and want a one-command bootstrap for new contributors |
| **Switch to Jarvy native** | No | None (jarvy manages hooks directly) | You want to drop the npm dependency for hooks, or your repo isn't a Node project |

The rest of this doc walks through each path.

> **Note:** Pre-commit framework support was removed in v0.8. Earlier versions of this doc included a "switch to pre-commit" path — use the native handler instead, which achieves the same goal (jarvy manages hooks, no npm/husky dependency) without pulling in a third-party framework.

---

## Path 1 — Wrap Husky (zero migration)

Jarvy can drive Husky as a first-class framework. Your `.husky/` directory and existing hooks stay exactly where they are; the only change is that `jarvy setup` (and `jarvy hooks install`) now bootstraps Husky on a fresh clone for you.

### Setup

```toml
# jarvy.toml
[git_hooks]
framework = "husky"     # explicit; otherwise auto-detected from .husky/
auto_install = true     # run `npx husky install` during `jarvy setup`
```

That's it. Existing hooks under `.husky/pre-commit`, `.husky/commit-msg`, etc. are unchanged. A fresh clone now runs:

```bash
jarvy setup
# → npm install --save-dev husky
# → npx husky install        (writes .husky/_/ + sets core.hooksPath)
```

…and the hooks fire on every `git commit` as before.

### What changes vs. running Husky directly

| Thing | Husky alone | Husky via Jarvy |
|---|---|---|
| `npm install` runs `husky install` | Via `package.json` `prepare` script | Same — Jarvy doesn't remove this; both paths converge |
| New contributor onboarding | `npm install` (assuming they read the README) | `jarvy setup` from a single command in the bootstrap script |
| Husky version updates | `npm install --save-dev husky@latest` by hand | `jarvy hooks update` runs the same thing |
| `jarvy hooks list` | n/a | Enumerates `.husky/*` files (skipping `_/` scaffolding) |
| `jarvy hooks run` | n/a | Runs every `.husky/<name>` script in lex order |
| CI integration | Whatever your pipeline does | `jarvy setup` puts Husky on a known version everywhere |

### Caveats

- **`package.json` is required.** Husky lives in npm dependencies; if you don't have a `package.json`, you can't use this path.
- **`npx` runs every commit.** Husky's overhead is real (~50-200ms per commit). If that matters, look at Path 2.

---

## Path 2 — Switch to Jarvy native

Move your hook scripts out of `.husky/` and into a folder (or a shared git repo) that jarvy scans directly. No npm, no husky, no third-party framework — jarvy writes the scripts straight into `.git/hooks/<stage>`.

### Setup

Move your existing `.husky/*` scripts into `scripts/hooks/` (or wherever you like — the folder name doesn't matter):

```
your-repo/
├── jarvy.toml
├── package.json                # keep or delete, doesn't affect hooks anymore
└── scripts/
    └── hooks/
        ├── pre-commit          # was .husky/pre-commit
        ├── pre-push            # was .husky/pre-push
        └── commit-msg          # was .husky/commit-msg
```

```toml
# jarvy.toml
[git_hooks.native]
dir = "scripts/hooks"
```

Then:

```bash
jarvy hooks install
# → writes scripts/hooks/pre-commit → .git/hooks/pre-commit
# → same for pre-push, commit-msg
# → each stamped with a "# managed by jarvy" marker
# → chmod +x
```

Remove the `.husky/` directory and the husky devDependency from `package.json` (or delete the whole package.json if it existed only for husky).

### Sharing hooks across many repos

If your organization has one hooks library used across many repos, publish it as a git repo and point at it:

```toml
[git_hooks.native]
repo = "github:your-org/team-hooks"
ref  = "v1.0.0"                # required — tag or SHA
```

Jarvy clones once, caches at `~/.jarvy/git_hooks_cache/<hash>/`, and refreshes idempotently on subsequent runs. See [Git hooks](git-hooks.md) for the full config surface (subpath, mixing with local `dir` / inline overrides, etc.).

### What changes vs. running Husky

| Thing | Husky | Jarvy native |
|---|---|---|
| Hook location | `.husky/<stage>` | `scripts/hooks/<stage>` (or wherever `dir` points) |
| `.git/hooks/` path | `.husky/_` via `core.hooksPath` | Native `.git/hooks/<stage>` |
| Per-commit overhead | ~50-200ms (spawns node) | None (git invokes the script directly) |
| npm dependency | Yes (`husky` in devDependencies) | No |
| Fresh-clone install | `npm install` (via `prepare` script) | `jarvy setup` (or `jarvy hooks install`) |
| Cross-repo hook sharing | Copy scripts, or an npm package | `[git_hooks.native] repo = "..."` |
| Overwrite protection | Husky manages `.husky/_` | Marker check; refuses to clobber hand-authored `.git/hooks/*` |

### Caveats

- **Contributors on a fresh clone must run `jarvy setup` or `jarvy hooks install` explicitly.** Husky's `npm install` hook is convenient because `npm install` was going to run anyway. With native hooks, the setup step is a separate command. The [bootstrap script](../scripts/bootstrap.sh) handles this — copy it into your repo's `scripts/` folder and contributors run one command.
- **No `--hook-stage` semantics.** Native hooks run whatever git's hook mechanism dispatches them to. If you were using Husky's stage grouping, split into separate stage files.

---

## Path 3 — Switch to lefthook

Not yet supported — the lefthook framework has a stub in jarvy today but the handler isn't wired. Track under `prd/048-pre-commit-hook-installation.md`. In the meantime, run lefthook directly (`lefthook install`) and set `[git_hooks] enabled = false` in `jarvy.toml`.

---

## Which one should you pick?

- **Node monorepo with a mature Husky setup already:** Path 1 (wrap it, zero migration).
- **Polyglot repo, want fewer moving parts, don't care about the migration cost:** Path 2 (native).
- **Small script-heavy repo where the hook is just "run cargo fmt --check":** Path 2 (inline body in `[git_hooks.native.hooks]`, no scripts folder at all).
- **Team hooks library shared across 10+ repos:** Path 2 with `repo = "..."`.

All paths honor the standard `[git_hooks] allow_remote` trust gate — a remote config (`jarvy setup --from <url>`) cannot land any of these without explicit opt-in.

## Related

- [Git hooks](git-hooks.md) — full reference for `[git_hooks]` including all `[git_hooks.native]` source shapes
- [Configuration reference](configuration.md) — schema
