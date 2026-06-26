---
title: "Cookbook — Jarvy"
description: "Task-oriented recipes for real-world Jarvy setups: corporate proxies, multi-project services, monorepo roles, secret managers, CI matrices, and webhook notifiers."
tags:
  - cookbook
---

# Cookbook

Recipes for the situations that show up in real teams. Each page is a self-contained walkthrough you can adapt.

<div class="grid cards" markdown>

-   :material-shield-lock-outline:{ .lg .middle } **Corporate proxy & TLS**

    ---

    Get `jarvy setup` working through your company's HTTP proxy with custom CA bundles.

    [:octicons-arrow-right-24: Behind a corporate proxy](corporate-proxy.md)

-   :material-database-outline:{ .lg .middle } **Multi-project Postgres**

    ---

    Run different Postgres versions for different projects on the same laptop without port conflicts.

    [:octicons-arrow-right-24: Side-by-side Postgres](multi-project-postgres.md)

-   :material-source-branch:{ .lg .middle } **Monorepo with multiple roles**

    ---

    Frontend, backend, and DevOps contributors share one repo but install only the tools they need.

    [:octicons-arrow-right-24: Monorepo role split](monorepo-roles.md)

-   :material-key-variant:{ .lg .middle } **Secrets via 1Password**

    ---

    Pull `[env.secrets]` values from 1Password CLI instead of hardcoding or relying on each developer to set them.

    [:octicons-arrow-right-24: 1Password integration](secrets-1password.md)

-   :material-source-branch-check:{ .lg .middle } **GitHub Actions matrix**

    ---

    Run `jarvy validate` and `jarvy drift check` across macOS, Linux, and Windows in CI.

    [:octicons-arrow-right-24: GitHub Actions matrix](github-actions-matrix.md)

-   :material-bell-ring-outline:{ .lg .middle } **Slack notifier hook**

    ---

    Notify a Slack channel when a contributor completes `jarvy setup` so the team can welcome them.

    [:octicons-arrow-right-24: Slack notifier](slack-notifier.md)

</div>

---

## Recipe template

Every recipe follows the same shape:

1. **Problem** — what real situation triggered this recipe
2. **Config** — the `jarvy.toml` snippet
3. **Why it works** — what each piece does
4. **Variations** — common tweaks
5. **Caveats** — when this doesn't fit

If you have a recipe to contribute, [open a PR](https://github.com/Cliftonz/jarvy/blob/main/docs/cookbook/) — recipes are the highest-leverage contribution to the docs.
