---
title: "Recipe: Slack notifier hook — Jarvy"
description: "Notify a Slack channel when a contributor finishes `jarvy setup` so the team can welcome new hires automatically."
tags:
  - cookbook
  - hooks
  - slack
---

# Recipe: Slack notifier hook

## Problem

You want the team to know when someone's environment is up. Maybe to welcome a new contributor, maybe to flag drift, maybe just so the lead engineer can sip coffee and watch onboardings tick by. Send a Slack message at the end of `jarvy setup` — without making it spam or blocking the install on Slack outages.

---

## Config

```toml title="jarvy.toml"
[hooks]
post_setup = """
# Skip in CI — only notify on real laptops.
if [ -n "${CI:-}" ]; then exit 0; fi

# Slack webhook URL is per-team; sourced from env, never hardcoded.
if [ -z "${JARVY_SLACK_WEBHOOK:-}" ]; then exit 0; fi

USER_NAME="${JARVY_USER:-${USER:-someone}}"
HOST_NAME="$(hostname -s)"
PROJECT="$(basename "$PWD")"

curl -s -X POST -H 'Content-Type: application/json' \\
    --data "{\\"text\\":\\":wave: $USER_NAME finished \\`jarvy setup\\` for \\`$PROJECT\\` on $HOST_NAME\\"}" \\
    "$JARVY_SLACK_WEBHOOK" \\
    || true   # never fail setup on a Slack outage

exit 0
"""

[hooks.config]
continue_on_error = true   # advisory: setup succeeds even if Slack 500s
timeout = 10
```

---

## Why it works

| Piece | What it does |
|---|---|
| `if [ -n "$CI" ]; then exit 0; fi` | CI runs don't notify — only laptop runs do. |
| `if [ -z "$JARVY_SLACK_WEBHOOK" ]; then exit 0; fi` | If the webhook isn't set, the hook is a silent no-op. Developers who haven't opted in get nothing. |
| `\|\| true` | Slack down? Setup still succeeds. The notifier is advisory. |
| `continue_on_error = true` | Belt and suspenders — even if curl errors, setup keeps going. |
| `timeout = 10` | A misbehaving Slack endpoint can't hang setup forever. |

Each developer sets `JARVY_SLACK_WEBHOOK` in their shell rc:

```bash title="~/.zshrc"
export JARVY_SLACK_WEBHOOK="https://hooks.slack.com/services/T0/B0/xxxx"
```

Or stash it in 1Password and pull it via `op` — see [Secrets via 1Password](secrets-1password.md).

---

## Variations

**Notify only on first setup, not re-runs:**

```bash
[ -f .jarvy/state.json ] && exit 0   # already set up, skip
```

**Notify on drift instead of setup:**

```toml
[hooks]
post_setup = """
DRIFT=$(jarvy drift check --format json 2>/dev/null || true)
if echo "$DRIFT" | grep -q '"drifted": true'; then
    curl ... "drift detected on $USER's $HOST"
fi
"""
```

**Discord / Teams / Mattermost:** same pattern, different webhook URL and JSON shape. Most chat platforms accept `{ "text": "..." }`.

**Channel routing by project:**

```bash
case "$(basename $PWD)" in
    frontend-*)  CHANNEL="#frontend-onboarding" ;;
    backend-*)   CHANNEL="#backend-onboarding" ;;
    *)           CHANNEL="#general-onboarding" ;;
esac
```

Then post to a Slack app that routes by channel name in the JSON.

**Track time-to-setup as a metric:**

```bash
DURATION=$(jq '.duration_seconds' .jarvy/state.json)
curl ... "$USER finished setup in ${DURATION}s"
```

---

## Caveats

- **Webhook URLs are secrets.** Don't commit them. Use the env-var pattern.
- **Don't notify in CI.** New runners on every PR = Slack channel firehose. Always guard with `if [ -n "$CI" ]; then exit 0; fi`.
- **Rate limits matter.** Slack incoming webhooks cap at 1 message/sec. If your hook fires for many tools, batch into one message.
- **Privacy:** the message includes `$USER` and hostname. If your team is privacy-conscious, drop those.

---

## See also

- [Hooks guide](../hooks.md)
- [Hook execution model](../concepts/hooks-execution.md)
- [1Password recipe](secrets-1password.md) — for the webhook URL
