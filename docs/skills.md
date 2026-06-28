---
title: "AI agent skills — Jarvy"
description: "Install reusable Claude Code / Cursor / Codex skills from a library manifest URL. Per-agent narrowing, sha-verified content, drift detection."
tags:
  - guides
  - skills
  - ai-hooks
---

# AI agent skills

Skills are markdown files (`SKILL.md`) that live under each AI coding agent's config directory and tell the agent how to handle specific tasks. `jarvy skills` lets a team publish skills at an HTTPS URL and have every developer's machine install them automatically during `jarvy setup`.

This is PRD-049 riding on the [library registry](library-registry.md) (PRD-054).

---

## Quick start

### Publisher

Add a `skill` item to your library manifest (see [library registry](library-registry.md) for the full schema):

```json
{
  "kind": "skill",
  "name": "myorg-code-review",
  "version": "2.1.0",
  "description": "MyOrg-specific code review checklist",
  "skill_md_url": "https://cdn.myorg.com/jarvy/skills/code-review-2.1.0/SKILL.md",
  "skill_md_sha256": "abc123...",
  "supported_agents": ["claude-code", "cursor", "codex"]
}
```

### Consumer (`jarvy.toml`)

```toml
[skills]
# auto-detect every AI agent installed on disk
# (or set agents = ["claude-code", "cursor"] to narrow)

# Option A — manifest URL (PRD-054):
[[skills.library_sources]]
url = "https://cdn.myorg.com/jarvy/manifest.json"

# Option B — Git repo with SKILL.md files (PRD-055):
[[skills.library_sources]]
url = "github:anthropics/skills@v1.0.0"
# Or fully qualified:
# url = "git+https://github.com/myorg/jarvy-skills.git@v1.2.0#skills/"

[skills.install]
myorg-code-review = "2.1.0"
myorg-debug-checklist = { version = "1.0.0", agents = ["claude-code"] }
```

Both source types appear in the same `library_sources` array — Jarvy fetches each according to its URL scheme. See [library registry](library-registry.md#git-sources-prd-055) for the full git-source surface (pin policy, SKILL.md frontmatter requirements, subpath safety).

Then:

```bash
jarvy skills install            # install every configured skill
jarvy skills list               # show what's configured + per-agent status
jarvy skills status             # drift summary
jarvy skills agents             # which AI agents jarvy detected
```

`jarvy setup` also installs skills automatically (gated on `[skills] auto_install = true`, which is the default).

---

## How install works

For each `(skill_name, version)` in `[skills.install]`:

1. Resolve via `library_registry::resolve_skill(name)` — looks up the matching item across every cached `library_sources` manifest.
2. Refuse if the requested `version` doesn't match the library item's `version` (no silent version drift).
3. Fetch `SKILL.md` over HTTPS (bounded read, 1 MiB cap).
4. **sha256-verify** the fetched body against the manifest's `skill_md_sha256`. Mismatch is fatal — the file does not land on disk.
5. Write to every target agent's `~/.<agent>/skills/<skill-name>/SKILL.md`.
6. Drop a `.jarvy-skill.json` sidecar recording version + sha256 + install time, so `jarvy skills status` can detect drift.

Use `"latest"` instead of an explicit version to pull whatever the library currently advertises.

---

## Agent paths

| Agent | Path |
|-------|------|
| Claude Code | `~/.claude/skills/<name>/SKILL.md` |
| Cursor | `~/.cursor/skills/<name>/SKILL.md` |
| Codex | `~/.codex/skills/<name>/SKILL.md` |
| Windsurf | `~/.windsurf/skills/<name>/SKILL.md` |
| Cline | `~/.cline/skills/<name>/SKILL.md` |
| Continue | `~/.continue/skills/<name>/SKILL.md` |

Jarvy treats `~/.{agent}/` existence as proof the agent is installed. `jarvy skills agents` shows what it detected.

Project-scope skills (`./.{agent}/skills/`) are NOT supported in v1 — only user-scope. PRD-049 follow-up.

---

## Per-skill agent narrowing

The bare-string form installs to every detected agent:

```toml
[skills.install]
myorg-code-review = "2.1.0"
```

The detailed form narrows to a subset:

```toml
[skills.install]
claude-only-skill = { version = "1.0.0", agents = ["claude-code"] }
```

There are two layers of narrowing that both apply:

1. **Consumer narrowing** — `agents = [...]` on the entry (this `jarvy.toml`)
2. **Publisher constraint** — `supported_agents = [...]` on the library item

A skill installs to an agent only when it passes both. Skipped agents are reported per skill in `jarvy skills install` output, with a reason.

---

## Status + drift

```
$ jarvy skills status
Skills Status
=============
Installed: 4
Missing:   1
Drift:     0

Run `jarvy skills install` to install missing skills.
```

`jarvy skills list` gives the per-skill, per-agent breakdown:

```
Configured skills (2):

  myorg-code-review = 2.1.0
    claude-code → installed (2.1.0)
    cursor → installed (2.1.0)

  myorg-debug-checklist = 1.0.0
    claude-code → missing
```

Drift is detected via the `.jarvy-skill.json` sidecar — when a user manually edits `SKILL.md` and the recorded sha256 no longer matches, the sidecar still records the installed version but the next install will overwrite (after re-verifying the fetched body's sha).

---

## Trust + safety

Skills fetched via `library_sources` carry the same trust model as every other library item — see [library registry trust model](library-registry.md#trust-model).

The short version:

- Remote `jarvy.toml` files (`jarvy setup --from <url>`) CANNOT declare `[[skills.library_sources]]`. Refused with `library.remote_refused` event.
- Every `SKILL.md` body is sha256-verified against the manifest. Mismatch refuses install.
- HTTPS-only.
- Cosign signature verification: scaffolded but not enforced in v1. Assume a publisher can ship arbitrary `SKILL.md` content until cosign enforcement lands.

---

## What's not in v1

Tracked under PRD-049 follow-up:

- `jarvy skills search` / `info` / `update` / `remove` subcommands
- Companion file fetching (today only `SKILL.md` lands; templates / scripts skip)
- Project-scope skills (`./.{agent}/skills/`)
- skills.sh API integration (search, popular, info)
- Version-range pinning (today only exact or `"latest"`)

---

## Related

- [Library registry](library-registry.md) — the underlying manifest + fetch + cache mechanism
- [AI hooks](ai-hooks.md) — sibling consumer; same `library_sources` shape
- [MCP registration](mcp-registration.md) — sibling consumer; same shape
- [Configuration reference](configuration.md) — full `[skills]` schema
