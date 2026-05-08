---
title: "Recipe: Secrets via 1Password — Jarvy"
description: "Pull `[env.secrets]` values from 1Password CLI so contributors never see plaintext tokens or hardcode credentials."
tags:
  - cookbook
  - secrets
  - 1password
---

# Recipe: secrets via 1Password

## Problem

Your project needs a `DATABASE_URL`, a `STRIPE_SECRET_KEY`, and an `OPENAI_API_KEY`. You don't want them in `jarvy.toml`, you don't want them in `.env` committed to git, and you don't want every new hire to spend an hour collecting them from Slack DMs. 1Password CLI has them; let `jarvy setup` pull them from there.

---

## Config

```toml title="jarvy.toml"
[provisioner]
op = "latest"   # 1Password CLI

[env.secrets]
DATABASE_URL     = { env = "DATABASE_URL", required = true }
STRIPE_SECRET    = { env = "STRIPE_SECRET", required = true }
OPENAI_API_KEY   = { env = "OPENAI_API_KEY", required = false }

[hooks]
pre_setup = """
# Sign in to 1Password (interactive, once per session).
if ! op whoami >/dev/null 2>&1; then
    eval $(op signin)
fi

# Inject project secrets into the current shell so [env.secrets] resolves.
# Each line: export VAR=$(op read "op://vault/item/field")
export DATABASE_URL=$(op read "op://Engineering/myapp-dev/database-url")
export STRIPE_SECRET=$(op read "op://Engineering/myapp-dev/stripe-secret")
export OPENAI_API_KEY=$(op read "op://Engineering/myapp-dev/openai-api-key")
"""
```

---

## Why it works

| Piece | What it does |
|---|---|
| `op = "latest"` in `[provisioner]` | Installs 1Password CLI on every laptop |
| `pre_setup` hook | Runs *before* tools install, so `op signin` happens first |
| `op read "op://..."` | Reads a single field from a vault item; no plaintext on disk |
| `[env.secrets] required = true` | Jarvy fails loudly if a secret didn't get exported |

Result: `jarvy setup` prompts for 1Password unlock once, then pulls every secret automatically. The values land in `.env` and the developer's shell rc.

---

## Variations

**Use `op inject` for templated `.env` files:**

```bash title=".env.tpl"
DATABASE_URL=op://Engineering/myapp-dev/database-url
STRIPE_SECRET=op://Engineering/myapp-dev/stripe-secret
```

```toml title="jarvy.toml"
[hooks]
post_setup = "op inject -i .env.tpl -o .env"
```

This is cleaner if you have many secrets — one source of truth, fewer shell exports.

**Per-developer secrets (Bitwarden, Vault, age):**

The same pattern works for any CLI-based secret store:

```toml
[hooks]
pre_setup = """
export DATABASE_URL=$(bw get item myapp-dev | jq -r '.fields[] | select(.name=="database-url") | .value')
"""
```

**Service account tokens (CI):**

```toml
[env.secrets]
DATABASE_URL = { env = "DATABASE_URL", required = true }
```

In CI, set `DATABASE_URL` from the platform's secret manager (GitHub Actions, GitLab CI variables). No `op` needed — the env var arrives pre-populated.

---

## Caveats

- **`op signin` is interactive.** Works for `jarvy setup` on a laptop, doesn't work in CI. Guard with `if [ -z "$CI" ]; then eval $(op signin); fi`.
- **Service accounts vs personal:** for CI, use 1Password service accounts (no human in the loop). For laptops, the developer's personal vault is fine.
- **Don't `echo` secrets in hooks.** Hook stdout is logged. Use `op read` directly into export, not `echo "$VAR"`.
- **Rotation is on you.** Jarvy reads secrets but doesn't manage their lifecycle. Pair with your secret manager's rotation policy.

---

## See also

- [Configuration reference — env.secrets](../configuration.md#environment-variables-env)
- [Hooks guide](../hooks.md)
- [1Password CLI docs](https://developer.1password.com/docs/cli)
