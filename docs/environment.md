---
title: "Environment variables — Jarvy"
description: "Declare env vars and secrets in jarvy.toml. Generate .env, update shell rc files, scope variables per tool, integrate with 1Password / Vault."
tags:
  - guides
  - configuration
---

# Environment variables

The `[env]` block of `jarvy.toml` declares the environment your project needs — runtime config (`NODE_ENV`, `DATABASE_URL`), per-tool overrides (`PYTHONPATH`, `GOPROXY`), and secret references that resolve at apply time. `jarvy env` then writes a `.env` file, updates your shell rc, or exports the values into the current shell.

This guide covers the four surfaces in order of trust: plain variables → tool-scoped overrides → user-prompted secrets → external secret managers.

---

## Plain variables

```toml
[env]
NODE_ENV     = "development"
DATABASE_URL = "postgres://localhost:5432/devdb"
LOG_LEVEL    = "debug"
```

Apply:

```bash
jarvy env --dotenv         # write/update .env in cwd
jarvy env --shell          # append to ~/.bashrc / ~/.zshrc / PowerShell profile
jarvy env --export         # print `export KEY=VALUE` to stdout (use with `eval`)
jarvy env --dry-run        # show what would change without writing
```

`--dotenv` is idempotent: existing `.env` entries with matching keys are updated in place, unrelated entries are left alone. Add `.env` to `.gitignore` — Jarvy does not commit it for you.

### Shell rc updates

`jarvy env --shell` writes a fenced block:

```bash
# >>> jarvy managed - do not edit by hand >>>
export NODE_ENV="development"
export DATABASE_URL="postgres://localhost:5432/devdb"
# <<< jarvy managed <<<
```

Re-running `jarvy env --shell` replaces the fenced block. Anything outside the fence is untouched. Use `--shell-type bash|zsh|fish|pwsh` to override auto-detection.

### Variable interpolation

```toml
[env]
HOME_BIN    = "$HOME/.local/bin"
NPM_PREFIX  = "${HOME_BIN}/npm-global"
PATH_PREPEND = "${NPM_PREFIX}:$PATH"
```

`$VAR` and `${VAR}` resolve at apply time using the current shell environment plus any earlier `[env]` entries. Forward references are an error.

---

## Tool-scoped variables

When a variable should only set for a single tool's lifetime (test runs, REPL, etc.):

```toml
[env.python]
PYTHONDONTWRITEBYTECODE = "1"
PYTHONUNBUFFERED        = "1"

[env.go]
GOPROXY = "https://proxy.golang.org,direct"
GOSUMDB = "sum.golang.org"
```

These do NOT land in `.env` or your shell rc. They are documented and surfaced by `jarvy env --show` and consumed by tooling that respects per-tool env (CI workflows, Make targets, hook scripts) via `jarvy env --tool python --export`.

---

## Secrets

Secrets are never embedded in `jarvy.toml`. The block declares the SHAPE; the values are resolved at apply time from one of three sources.

### Interactive prompt

```toml
[env.secrets]
ANTHROPIC_API_KEY = { prompt = "Enter your Anthropic API key" }
GITHUB_TOKEN      = { prompt = "Paste a GitHub PAT (repo, workflow scopes)" }
```

`jarvy env --dotenv` (or `--shell`) prompts each unset secret once. The entered value lands in `.env` / shell rc; the prompt does not repeat on subsequent runs unless `--force` is passed.

Auto-disable in unattended environments: when `jarvy ci-info` detects CI or a sandbox, prompts are replaced by warnings — secrets stay unset rather than blocking the run.

### Environment passthrough

```toml
[env.secrets]
NPM_TOKEN     = { from_env = "NPM_TOKEN" }
SENTRY_DSN    = { from_env = "SENTRY_DSN", fallback = "https://localhost/0" }
```

Reads the value from the current process env at apply time. Use this in CI where the token is already exported by the workflow. `fallback` is used only when the source env var is unset; it's intended for non-secret defaults.

### External secret managers

```toml
[env.secrets]
DB_PASSWORD = { op = "op://Vault/devdb/password" }                       # 1Password CLI
JWT_SIGNING = { vault = "secret/data/dev/jwt", field = "signing_key" }   # HashiCorp Vault
AWS_KEY     = { aws_secret = "dev/aws-key" }                             # AWS Secrets Manager
```

Jarvy shells out to the respective CLI (`op read`, `vault kv get -field=...`, `aws secretsmanager get-secret-value`). If the CLI is missing or auth fails, the secret stays unset and the run continues with a warning. See [cookbook: 1Password secrets](cookbook/secrets-1password.md) for the full setup.

---

## Apply during `jarvy setup`

`jarvy setup` automatically runs the equivalent of `jarvy env --dotenv` after tool installation, before services start. This means a `.env`-aware app (Next.js, dotenv-rails, godotenv) sees the right values when `auto_start = true` brings up its Compose stack.

Disable with:

```bash
jarvy setup --no-env       # skip [env] application
```

Or scope it:

```toml
[env]
# default scope is "dotenv"; set "shell" to land in rc instead, or "none" to require explicit `jarvy env`
apply_during_setup = "dotenv"
```

---

## Inspecting what's resolved

```bash
jarvy env --show                    # resolved values, secrets masked as `***`
jarvy env --show --reveal-secrets   # plain text; refuses in CI
jarvy env --diff                    # what jarvy env --dotenv would change
jarvy env --tool python --export    # tool-scoped only, ready for `eval`
```

`--reveal-secrets` requires an interactive TTY and prints a one-line warning. The audit log records the reveal with the user, source file, and resolved key list (values are not logged).

---

## Trust boundaries

`jarvy setup --from <url>` fetches a remote config. Remote configs MAY declare `[env]` but:

- `prompt` secrets are refused (no interactive prompts under remote configs)
- `op` / `vault` / `aws_secret` resolvers are refused unless `[env] allow_remote_secrets = true` is set in the SOURCE config (not on the consuming machine — the policy travels with the file)
- Plain variables and `from_env` passthrough are allowed

This mirrors the trust gates on `[ai_hooks]` and `[mcp_register]`: remote configs may narrow trust, never broaden it.

---

## Troubleshooting

- **`.env` keeps being deleted by my editor** — disable "Auto-save formatted file" for `.env` in VS Code, or move to `.env.local` (Next.js / Vite already prefer it).
- **Variable interpolation produces literal `$HOME`** — your shell is not POSIX-compatible (cmd.exe). Use `--shell-type pwsh` or write the value out fully.
- **1Password / Vault resolvers fail silently** — run `jarvy env --show` with `JARVY_LOG=debug` to see the underlying CLI invocation and stderr tail.

---

## Next

- [Configuration reference](configuration.md) — full `[env]` schema
- [Cookbook: 1Password secrets](cookbook/secrets-1password.md) — end-to-end setup
- [Cookbook: corporate proxy](cookbook/corporate-proxy.md) — `HTTP_PROXY`, `NO_PROXY`, CA bundles
