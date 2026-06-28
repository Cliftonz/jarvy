---
prd: 055-git-skill-sources
title: Git-shorthand for skill sources — point [skills.library_sources] at a repo, not a manifest
version: 1.0
status: in_progress
priority: medium
estimated_days: 3
created: 2026-06-28
---

# PRD-055: Git-shorthand for skill sources

## Overview

Today [`[skills.library_sources]`](054-library-registry.md) requires the publisher to host a `manifest.json` describing each skill. For skills specifically — where SKILL.md carries its own YAML frontmatter metadata — this is unnecessary friction. PRD-055 lets a consumer point `library_sources` at a plain Git repository; Jarvy clones it, walks for `SKILL.md` files, and synthesizes a manifest in-memory.

```toml
# Today (PRD-054):
[[skills.library_sources]]
url = "https://cdn.myorg.com/jarvy/manifest.json"

# After PRD-055:
[[skills.library_sources]]
url = "git+https://github.com/myorg/jarvy-skills.git@v1.2.0"

# Or shorthand:
[[skills.library_sources]]
url = "github:myorg/jarvy-skills@v1.2.0"
```

## Problem Statement

Publishing skills via PRD-054 requires:

1. Computing sha256 for every `SKILL.md` body.
2. Writing + hosting `manifest.json` (and keeping it in lockstep with the actual files).
3. Bumping the version + sha on every content edit.

For a team that already has a Git repo of skills (common pattern — see [Anthropic's skills repo](https://github.com/anthropics/skills) or anyone forking it), this is busywork. Git already provides:

- **Content addressing** — commit SHA is a tamper-evident pin.
- **Versioning** — tags + branches + commits map naturally to `ref =` pins.
- **History** — `git log` answers "what changed?" without a manifest diff.
- **Hosting** — GitHub / GitLab / Bitbucket / self-hosted — all already serve HTTPS.

The right shape: let consumers reference a Git repo + ref, scan SKILL.md frontmatter, build an equivalent manifest in-process. Publishers write a repo. No `manifest.json` discipline required.

## Why skills first (and only)

SKILL.md has standard YAML frontmatter (`name`, `description`, `globs`, etc.) — Jarvy already parses this when installing. The metadata to build a manifest entry is already in the file.

AI hooks (`bash:` script bodies) and MCP server entries (command / args / env tables) have no equivalent self-describing format. Inferring them from a Git tree would require Jarvy to invent a convention — out of scope.

If a publisher wants to ship AI hooks + MCP servers from a Git repo, they put a `manifest.json` at the root and use the existing PRD-054 flow. This is documented as the canonical answer.

## Goals

1. **`git+https://...` URL scheme** in `library_sources.url` — auto-routes to a git fetcher.
2. **`github:org/repo` shorthand** — same as above for the common case.
3. **Pinning** — `@<ref>` (tag, branch, or commit SHA) is mandatory. Drift-prone unpinned refs are refused at parse time.
4. **Subpath** — `#<path>` lets a repo contain non-skill content (`README.md`, `LICENSE`, etc.) alongside the skills directory.
5. **No new tools required** — fetches use `git` CLI (already in 235+ tool registry, and present on most dev machines).
6. **Cache parity** — git-fetched libraries cache to the same `~/.jarvy/library.d/` tree as URL-fetched ones, keyed by the same `sha256(url)`.

## Non-Goals

- AI hook / MCP server discovery from Git trees (publishers ship a `manifest.json` for those).
- SSH auth (`git+ssh://`). HTTPS-only; private repos use the developer's git credential helper.
- Sparse checkout. v1 clones the whole repo; large repos are slow. Sparse-checkout is a v2 optimization.
- libgit2 / git2-rs dependency. Shell out to `git`; refuse with a clear error if it's missing.
- Push / write operations. Read-only mirror.

## URL formats

### `git+https://` (RFC 3986-style canonical form)

```
git+https://github.com/myorg/jarvy-skills.git@v1.2.0#skills/
                                              ↑     ↑
                                              ref   subpath
```

- Scheme: `git+https://` (HTTPS-only; `git+http://` refused except loopback under test bypass).
- Authority + path: standard git HTTPS clone URL.
- `@<ref>`: tag, branch, or commit SHA. **Required.** Unpinned URLs refused at parse time with a clear message.
- `#<subpath>`: optional path within the repo to scan. Defaults to repo root. Trailing slash optional.

### `github:` shorthand

```
github:myorg/jarvy-skills@v1.2.0#skills/
```

Equivalent to `git+https://github.com/myorg/jarvy-skills.git@v1.2.0#skills/`. Convenience only.

### Future shorthand (not in v1)

- `gitlab:org/repo` — same shape, different host.
- `bitbucket:org/repo` — same shape, different host.
- `git+ssh://...` — SSH auth.

v1 ships HTTPS only. The dispatch is centralized so adding more is a small follow-up.

## SKILL.md frontmatter convention

Each `SKILL.md` under the scanned subpath becomes one skill item. Frontmatter shape:

```markdown
---
name: code-review                    # required — used as skill identifier
version: 2.1.0                       # required — used as the manifest version
description: MyOrg code-review checklist
supported_agents:                    # optional; default = all
  - claude-code
  - cursor
---

# Code Review Skill

(body — anything the agent should read)
```

Fields missing on a SKILL.md cause the file to be skipped with a `library.git_skill.skipped` event citing the reason (`missing_name`, `missing_version`, etc.). Publishers see this on the next `jarvy skills install` — no silent failures.

## Trust model

Inherits PRD-054 wholesale:

| Config origin | git source allowed? |
|---|---|
| Local | Yes |
| Remote (`jarvy setup --from <url>`) | No — refused with `library.remote_refused` event |

**No additional gate for git-vs-HTTP**. The trust model is "do you trust the URL." Git fetch + manifest fetch are equivalent here — both pull arbitrary content from a remote into a process that will write to `~/.{agent}/skills/`.

**No sha256 on fetched SKILL.md**. The trust anchor is the `@<ref>` pin: a commit SHA is tamper-evident, a tag is mutable (publishers can re-tag, this is documented as a risk), a branch is freely mutable (documented + a warning at fetch). For the strongest guarantee, pin to a commit SHA.

`jarvy library list` (when shipped) surfaces the ref the cache currently holds, so operators can diff "what's pinned in jarvy.toml" against "what's actually on disk."

## Cache layout

```
~/.jarvy/library.d/
  <sha256-of-url>/
    manifest.json                  # synthesized from SKILL.md frontmatter
    git/
      <cloned repo tree at ref>
        SKILL.md
        skills/
          code-review/
            SKILL.md
```

`<sha256-of-url>` is the full URL including `@<ref>#<subpath>`. Changing the ref produces a new cache directory; old caches age out via `jarvy library clean`.

## Implementation

### Module additions

```
src/library_registry/
├── git_fetch.rs          # NEW: clone + walk + synthesize Manifest
├── url_parser.rs         # NEW: git+https:// + github: parsing
├── fetch.rs              # (existing) HTTPS manifest fetch
├── manifest.rs           # (existing) Manifest schema
├── mod.rs                # dispatcher: sync() routes by scheme
└── ...
```

### Public API additions

```rust
// src/library_registry/url_parser.rs
pub enum SourceScheme<'a> {
    Manifest { url: &'a str },              // existing: https://...
    Git { repo: String, git_ref: String, subpath: Option<String> },
}

pub fn parse_source(url: &str) -> Result<SourceScheme<'_>, LibraryError>;
```

`sync()` calls `parse_source(&source.url)` and dispatches:

```rust
match parse_source(&source.url)? {
    SourceScheme::Manifest { url } => fetch_manifest(url, source)?,
    SourceScheme::Git { repo, git_ref, subpath } => {
        git_fetch::sync_git(&repo, &git_ref, subpath.as_deref(), source)?
    }
}
```

### `git_fetch::sync_git`

1. Validate `git` is on PATH; return `LibraryError::Tool("git", ...)` if missing.
2. Compute cache dir from `sha256(source.url)`.
3. If cache dir exists, refresh via `git fetch + git checkout <ref>` (cheap when ref hasn't moved).
4. Else `git clone --depth 1 --branch <ref> <repo> <cache_dir>/git/` (falls back to full clone if `--branch` doesn't match — e.g. commit SHAs can't be branch-shallow-cloned).
5. Walk `<cache_dir>/git/<subpath>/**/SKILL.md`.
6. For each SKILL.md, parse YAML frontmatter; build a `LibrarySkillItem` with `skill_md_url = "file://<cache-path>"` and `skill_md_sha256 = sha256(body)`.
7. Construct an in-memory `Manifest { publisher = repo, items: [...] }` and write it to `<cache_dir>/manifest.json`.
8. Populate the in-process cache same as `fetch_manifest`.

### Installer integration

The skill installer already calls `fetch_bounded(item.skill_md_url, ...)`. For git-fetched items, `skill_md_url` is a `file://` URL pointing into the cache. Add a branch to `fetch_bounded` (or a thin wrapper at the installer site) that recognizes `file://` and reads from disk instead of HTTPS.

Alternative considered + rejected: inline the SKILL.md body into the synthesized manifest item. Pro: avoids the file:// branch. Con: 1 MiB cap on item size; large skill bodies break. The file:// path scales.

### `git` subprocess invocation

```rust
fn git_clone(repo: &str, git_ref: &str, dest: &Path) -> Result<(), LibraryError> {
    // Step 1: shallow clone of the default branch (cheap).
    Command::new("git")
        .args(["clone", "--depth", "1", "--no-tags", repo, dest_str])
        .status()?;
    // Step 2: fetch the specific ref (works for tags, commits, branches).
    Command::new("git")
        .args(["fetch", "--depth", "1", "origin", git_ref])
        .current_dir(dest)
        .status()?;
    // Step 3: checkout.
    Command::new("git")
        .args(["checkout", "--detach", "FETCH_HEAD"])
        .current_dir(dest)
        .status()?;
    Ok(())
}
```

Three commands instead of one `clone --branch` because `clone --branch` doesn't accept commit SHAs. The fetch-then-checkout pattern handles tags / branches / SHAs uniformly.

### Telemetry

| Event | When | Fields |
|---|---|---|
| `library.git.clone_started` | begin git clone | `repo` (redacted), `ref` |
| `library.git.clone_completed` | clone OK | `repo`, `ref`, `subpath`, `skills_discovered`, `duration_ms` |
| `library.git.clone_failed` | git exit nonzero | `repo`, `ref`, `error` |
| `library.git.cache_hit` | served from cache without re-clone | `repo`, `ref`, `age_seconds` |
| `library.git_skill.skipped` | SKILL.md missing required frontmatter | `path`, `reason` |
| `library.git.missing_git` | `git` CLI not on PATH | `os` |

## Trade-offs

### Trust drift on mutable refs

A publisher who pins `@main` accepts that re-running `jarvy skills install` after the publisher pushes a commit will install different content under the same skill name and version. This is exactly Git's semantics; we document it but do not refuse.

Recommended pinning practice (documented):

| Ref type | Trust posture |
|---|---|
| Commit SHA (`@abc1234`) | Tamper-evident. Recommended for production. |
| Tag (`@v1.2.0`) | Mutable (publishers can re-tag). Recommended for ergonomics + version visibility. |
| Branch (`@main`) | Freely mutable. Emit warning event every fetch. Documented as dev-only. |

### `git` CLI dependency

Most dev machines have `git`. Refusing with a clear error when it's missing is better UX than vendoring libgit2 / git2-rs (which would bloat binary size + maintenance surface). Tracked as a v2 enhancement if needed.

### Subpath traversal safety

`#<subpath>` is appended to the cloned tree's root. Refuse `..`, absolute paths, and any path component that escapes the clone via canonical-path check. Mirrors `safety::resolve_within_workspace` from `src/mcp/extended_tools.rs`.

## Migration

Additive. Existing PRD-054 manifest-based sources continue to work unchanged. Publishers can mix — a single `library_sources` array can contain both manifest URLs and git URLs.

## CLI

No new subcommands. The existing `jarvy skills install` / `list` / `status` work transparently.

`jarvy library show <publisher>` (PRD-054 phase 6) will surface the underlying source scheme so operators can tell git-fetched vs manifest-fetched at a glance.

## Implementation phases

| Phase | Scope | Effort |
|---|---|---|
| 1 | url_parser + git_fetch modules; sync() dispatch; file:// branch in fetch_bounded | 1.5d |
| 2 | Telemetry events + clear error messages | 0.5d |
| 3 | Tests (URL parsing, frontmatter walker, subpath traversal refusal) | 1d |
| 4 | Docs + changelog + PRD-054 cross-reference | 0.5d |
| **Total** | | **~3 days** |

v1 ships all four. No follow-up phases planned for PRD-055 itself — the work either fits or it doesn't.

## Success metrics

| Metric | Current | Target |
|---|---|---|
| Skills publishers required to maintain manifest.json | 100% | <50% (rest use git shorthand) |
| Time from "team has a SKILL.md in Git" to "everyone installs it" | hours (write manifest, host, refresh) | minutes (add `[[skills.library_sources]] url = "github:..."` to jarvy.toml) |

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `git` not installed | Low | Medium | Clear error message + install hint; document as a prereq |
| Slow first clone on large repos | Medium | Low | `--depth 1`; document sparse-checkout as v2 |
| Trust drift on mutable refs | Medium | Medium | Warn on branch pins; document the SHA > tag > branch hierarchy |
| Frontmatter parse failures | Low | Low | Per-file skip with `library.git_skill.skipped` event citing reason |
| Subpath escape | Low | High | Canonical-path check inside `safety::resolve_within_workspace`-style helper |

## Related

- PRD-054 — library registry (the foundation this rides on)
- PRD-049 — skills installation (the consumer this extends)
- `src/library_registry/fetch.rs` — existing HTTPS fetcher; v1 reuses redaction + bounded-read patterns
