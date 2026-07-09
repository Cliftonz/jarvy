---
title: "Git Configuration - Jarvy"
description: "Automate git identity, commit signing, defaults, line endings, credential helpers, and aliases across the team."
---

# Git Configuration

`[git]` lets `jarvy.toml` codify Git settings the same way it codifies tools. New developers get a correctly configured Git on first `jarvy setup` — no more "I forgot to set my email" PRs.

## Minimal Example

```toml
[git]
user_name = "Jane Doe"
user_email = { env = "GIT_EMAIL" }
default_branch = "main"
pull_rebase = true
```

`user_email = { env = "GIT_EMAIL" }` keeps personal email out of the shared config — each developer sets `GIT_EMAIL` in their shell.

## Full Configuration

```toml
[git]
# Identity
user_name = "Jane Doe"
user_email = { env = "GIT_EMAIL", default = "jane@example.com" }

# Commit signing
signing = true
signing_key = "~/.ssh/id_ed25519.pub"
signing_format = "ssh"            # ssh | gpg, auto-detected from key extension

# Defaults
default_branch = "main"
pull_rebase = true
auto_stash = true
push_autosetup = true
editor = "vim"

# Line endings
autocrlf = "input"                # true | false | input
eol = "lf"

# Credential helper (auto-detected per OS if omitted)
credential_helper = "osxkeychain"

# Scope
scope = "global"                  # global (~/.gitconfig) | local (.git/config)

# OS-aware defaults (enabled unless set to false)
os_defaults = true

# Aliases
[git.aliases]
co = "checkout"
br = "branch"
ci = "commit"
st = "status"
lg = "log --oneline --graph --decorate"
```

## ConfigValue Resolution

Any string field accepts three forms:

| Form | Example | Behavior |
|------|---------|----------|
| Plain | `user_name = "Jane"` | Used as-is |
| Env-only | `user_email = { env = "GIT_EMAIL" }` | Reads env at runtime; errors if unset |
| Env + default | `user_email = { env = "GIT_EMAIL", default = "fallback@x.com" }` | Reads env, falls back if unset |

Use the env+default form to keep secrets and personal info out of the shared `jarvy.toml`.

## Signing

Commit signing is auto-detected from the key extension:

| Key | Format detected |
|-----|-----------------|
| `~/.ssh/id_ed25519.pub` | `ssh` |
| `~/.ssh/id_rsa.pub` | `ssh` |
| Any other path | `gpg` |

Override explicitly with `signing_format`:

```toml
signing_format = "gpg"
```

When `signing = true`, Jarvy sets:

- `commit.gpgsign = true`
- `tag.gpgsign = true`
- `gpg.format = ssh|openpgp` based on `signing_format`
- `user.signingkey = <signing_key>`
- For SSH: configures `gpg.ssh.allowedSignersFile` if present

## Credential Helper Defaults

If `credential_helper` is omitted, Jarvy picks per OS:

| OS | Default |
|----|---------|
| macOS | `osxkeychain` |
| Linux | `cache` |
| Windows | `manager-core` |

Override with any helper name accepted by `git config credential.helper`.

## Scope

| Scope | File | Use |
|-------|------|-----|
| `global` (default) | `~/.gitconfig` | Per-developer settings |
| `local` | `.git/config` | Per-repo settings (e.g. work email for a work repo) |

A common pattern: keep `user_name`/`user_email` at scope `local` for a work repo, leave personal global config alone.

## Aliases

```toml
[git.aliases]
co = "checkout"
unstage = "reset HEAD --"
last = "log -1 HEAD"
```

These map directly to `git config --<scope> alias.<name> "<value>"`. Existing aliases are overwritten.

## OS-Aware Defaults

Like `credential.helper`, Jarvy fills in host-appropriate defaults for a few keys the user left unset. Enabled by default; set `os_defaults = false` to opt out.

| Key | Windows | macOS | Linux | Why |
|-----|---------|-------|-------|-----|
| `core.autocrlf` | `true` | `input` | `input` | CRLF↔LF conversion — Windows uses CRLF, Unix commits LF untouched |
| `core.longpaths` | `true` | — | — | Allow paths beyond the 260-char `MAX_PATH` limit |
| `core.precomposeunicode` | — | `true` | — | Recompose APFS/HFS+ NFD filenames to NFC for cross-platform matches |

Jarvy also applies a small set of **cross-platform recommended defaults** under the same `os_defaults` flag (unset keys only, `[git.extra]` still wins):

| Key | Value | Why |
|-----|-------|-----|
| `fetch.prune` | `true` | Drop local refs for branches deleted on the remote |
| `rerere.enabled` | `true` | Reuse recorded conflict resolutions on re-merge/rebase |
| `merge.conflictStyle` | `zdiff3` | Show the common base in conflict markers (needs git ≥ 2.35; older git ignores it) |

These are only written when the corresponding value is unset. An explicit typed field (e.g. `autocrlf = "false"`) or a `[git.extra]` entry for the same key always wins — Jarvy never overwrites an explicit value.

## Extra Keys (escape hatch)

For git config keys Jarvy doesn't model as first-class fields, use `[git.extra]`. Keys are dotted git config keys; values are written verbatim via `git config --<scope> <key> <value>`.

```toml
[git.extra]
"core.fsmonitor"     = "true"
"feature.manyFiles"  = "true"
"diff.colorMoved"    = "zebra"
"branch.main.rebase" = "true"
```

Rules and guardrails:

- Applied **last**, so an entry here overrides a modeled field targeting the same key.
- Keys must match the dotted grammar `section.key` / `section.subsection.key` with chars in `[A-Za-z0-9._-]`. Keys starting with `-`, missing a `.`, or with empty segments are refused (flag-injection guard). Keys needing `:` or `/` (e.g. `url.<base>.insteadOf`) are not supported by this map.
- Values starting with `!` are refused for **every** extra key — git would run them as a shell command. Prefer a modeled field for the few keys that legitimately need shell (none currently exposed).
- **Security guardrails** — values that weaken a git defense are refused: `core.protectNTFS`/`core.protectHFS` = false (`.git`-path smuggling), `safe.directory = *` (CVE-2022-24765 ownership check), and `fsck.* = ignore` (object-integrity checks). Set `JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1` to override deliberately.
- Prefer a typed field when one exists (`autocrlf`, `editor`, etc.); reach for `[git.extra]` only when there's no first-party analogue.

## What Runs

`jarvy setup` invokes `git config --<scope> <key> <value>` for each setting. The order:

1. Identity (`user.name`, `user.email`)
2. Signing config (if enabled)
3. Defaults (`init.defaultBranch`, `pull.rebase`, etc.)
4. Line endings (`core.autocrlf`, `core.eol`)
5. Credential helper
6. OS-aware + recommended defaults (`core.autocrlf`, Windows `core.longpaths`, macOS `core.precomposeunicode`, `fetch.prune`, `rerere.enabled`, `merge.conflictStyle` — unset keys only)
7. Aliases
8. Extra keys (`[git.extra]`, override-last)

If `git` itself is missing, the whole `[git]` section is skipped with a warning — install Git first.

## CLI

```bash
jarvy setup           # Applies [git] config
jarvy doctor          # Verifies expected values are set
jarvy diff            # Shows pending git config changes
```

## Module

- Source: `src/git/`
- Files: `config.rs`, `identity.rs`, `signing.rs`, `aliases.rs`, `setup.rs`
- Key types: `GitConfig`, `ConfigValue`, `ConfigScope`, `SigningFormat`, `AutoCrlf`
