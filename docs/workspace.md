# Monorepo workspaces (`jarvy workspace`)

`jarvy workspace` is the read-only inspection surface for monorepo
projects that declare a `[workspace]` block in their root `jarvy.toml`.

> **PRD-047 MVP.** Workspace-aware `jarvy setup --project <name>`
> orchestration is intentionally deferred — surfacing the resolved
> structure first lets users debug inheritance before we add a command
> that mutates the environment based on it.

## Declaring a workspace

```toml
# /repo/jarvy.toml — workspace root
[workspace]
members = ["apps/web", "apps/api", "packages/shared"]

# What sections members inherit from the root config. Empty list is
# treated as ["provisioner"] (the most common case) when inspecting
# resolved tools.
inherit = ["provisioner", "hooks"]

[provisioner]
git = "latest"
docker = "latest"
```

Each member directory may optionally have its own `jarvy.toml` that
adds tools or overrides inherited values:

```toml
# /repo/apps/web/jarvy.toml
[provisioner]
node = "20"
docker = "24.0"     # overrides workspace's docker = "latest"
```

```toml
# /repo/apps/api/jarvy.toml
[provisioner]
go = "1.21"
golangci-lint = "latest"
```

Members without their own `jarvy.toml` inherit the workspace defaults
unchanged.

## CLI

```bash
# Enumerate members + their resolved tool sets
jarvy workspace --file ./jarvy.toml list

# Show one member's resolved config (with inheritance applied + provenance)
jarvy workspace --file ./jarvy.toml show apps/web

# Validate that members exist on disk and their jarvy.toml parses
jarvy workspace --file ./jarvy.toml validate
```

All three subcommands accept `--format json` for AI agents / CI.

### Sample output

```text
$ jarvy workspace --file ./jarvy.toml list
Workspace: /repo
Inherits: provisioner, hooks
Members (3):
  [ok ] apps/web               docker, git, node
  [ok ] apps/api               go, golangci-lint
  [MISS] packages/shared       (uses workspace defaults)
```

```text
$ jarvy workspace --file ./jarvy.toml show apps/web
Project: apps/web
Path:    /repo/apps/web
Config:  /repo/apps/web/jarvy.toml
Inherits sections: provisioner, hooks

Tools (3):
  docker = "24.0" (overridden)
  git = "latest" (inherited)
  node = "20"
```

The `(overridden)` / `(inherited)` annotations come from comparing
the merged provisioner table to the raw root + member tables — gives
a direct answer to "where did this tool come from?"

```text
$ jarvy workspace --file ./jarvy.toml validate
Validating workspace at /repo
  warn: packages/shared: no jarvy.toml (workspace defaults apply)
  2 ok, 1 warning(s), 0 error(s).
```

Validate exits `0` when there are no errors (warnings are advisory)
and `CONFIG_ERROR` (2) when any member's directory is missing or its
jarvy.toml fails to parse.

## Inheritance semantics

Member configs merge with the root via
`crate::workspace::merge_configs`:

- For sections in `inherit`, member values **completely override** root
  values **except** for `provisioner`, which is merged tool-by-tool
  (member wins on conflict).
- For sections NOT in `inherit`, the member gets only what's in its
  own jarvy.toml.

If `inherit = []` (or omitted), `jarvy workspace show` / `list` treat
it as `["provisioner"]` for display so the most common monorepo case
works without extra config. The underlying `merge_configs` function
still honors the explicit empty-list semantics — only the `workspace`
CLI surface widens the default.

## What's deferred

Items from PRD-047 that v1 does NOT ship:

- Glob patterns in `[workspace] members` (e.g. `apps/*`). Exact paths
  only.
- `[workspace] exclude = [...]` patterns.
- Auto-context detection (running `jarvy setup` from a subdir
  automatically scoping to that member).
- `jarvy setup --project <name>` workspace-aware orchestration.
- A standalone `jarvy context` command.

Open an issue if you need any of these — the foundation is already in
place (`workspace::find_workspace_root` + `merge_configs`), so they're
small follow-ups.
