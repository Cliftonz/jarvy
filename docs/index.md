---
title: "Jarvy — Dev environments as code"
description: "Jarvy is a fast, cross-platform CLI that provisions a complete local development environment from a single jarvy.toml file. Native package managers, no cloud, MIT-licensed."
hide:
  - navigation
---

# Dev environments as code.

Stop writing onboarding wikis. Stop paying for cloud dev pods. Stop debugging "works on my machine."

**Jarvy reads one file, `jarvy.toml`, and gets every developer on your team to the same set of tools, the same versions, in seconds — on macOS, Linux, and Windows.**

<div class="grid cards" markdown>

-   :material-rocket-launch:{ .lg .middle } **Install in 30 seconds**

    ---

    ```bash
    curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
    ```

    Or `brew install jarvy` · `cargo install jarvy` · [binary](https://github.com/bearbinary/jarvy/releases)

-   :material-clock-fast:{ .lg .middle } **Provision in seconds**

    ---

    ```bash
    cd your-repo && jarvy setup
    ```

    Idempotent. Re-runnable. Detects what's already installed.

</div>

---

## See it in 30 seconds

<div id="jarvy-demo" style="margin: 1.5rem 0; max-width: 100%;"></div>

<script>
  document.addEventListener('DOMContentLoaded', function () {
    if (typeof AsciinemaPlayer !== 'undefined' && document.getElementById('jarvy-demo')) {
      AsciinemaPlayer.create(
        'assets/jarvy-setup.cast',
        document.getElementById('jarvy-demo'),
        { fit: 'width', terminalFontSize: 'medium', theme: 'asciinema', poster: 'npt:0:13', idleTimeLimit: 1 }
      );
    }
  });
</script>

```toml title="jarvy.toml"
[provisioner]
git    = "latest"
node   = "20"
python = "3.12"
docker = "latest"

[hooks.node]
post_install = "npm install -g typescript"

[env.vars]
NODE_ENV = "development"
```

```bash
$ jarvy setup
✓ git 2.45.0 already installed
✓ node 20.11.0 installed via brew
✓ python 3.12.1 installed via pyenv
✓ docker 25.0 installed via brew cask
✓ ran hook for node: npm install -g typescript
✓ wrote .env
Setup complete in 14.3s
```

That's the entire onboarding flow. Add it to `README.md`, push to `main`, every new hire is productive in one command.

---

## Why teams switch to Jarvy

<div class="grid cards" markdown>

-   :material-flash:{ .lg .middle } **Native, not virtual**

    ---

    Tools install directly on the laptop. No Docker daemon, no VM, no remote SSH. Your editor, debugger, and shell just work.

-   :material-cloud-off-outline:{ .lg .middle } **Zero cloud cost**

    ---

    No Codespaces tab. No Gitpod usage tier. No idle compute charges. Your laptop is the dev environment.

-   :material-source-branch-check:{ .lg .middle } **Git is the source of truth**

    ---

    `jarvy.toml` lives in your repo, reviewed in PRs, versioned with the code it supports. No drift between docs and reality.

-   :material-arrow-decision:{ .lg .middle } **Drift detection**

    ---

    Jarvy snapshots the environment after setup and tells you when a teammate's machine has wandered off the baseline.

-   :material-account-group:{ .lg .middle } **Roles for real teams**

    ---

    Frontend, backend, DevOps, data — each gets the tools they need, with inheritance and per-role overrides.

-   :material-robot-outline:{ .lg .middle } **Agent-native**

    ---

    Built-in MCP server lets Claude, Cursor, and ChatGPT discover, install, and configure tools the same way you do.

</div>

---

## Pick a starting point

<div class="grid cards" markdown>

-   :material-school-outline:{ .lg .middle } **New to Jarvy?**

    ---

    Walk through your first config in 5 minutes — install, configure, provision, verify.

    [:octicons-arrow-right-24: Tutorial: your first jarvy.toml](tutorials/first-config.md)

-   :material-account-multiple-plus:{ .lg .middle } **Onboarding a team?**

    ---

    Write a `jarvy.toml` for your repo and ship it to every contributor.

    [:octicons-arrow-right-24: Tutorial: onboard a team](tutorials/team-onboarding.md)

-   :material-book-open-page-variant:{ .lg .middle } **Learning the model?**

    ---

    Concepts, lifecycle, and how the pieces fit together.

    [:octicons-arrow-right-24: Concepts overview](concepts/overview.md)

-   :material-file-document-outline:{ .lg .middle } **Looking up syntax?**

    ---

    Every option in `jarvy.toml`, every CLI subcommand, every error code.

    [:octicons-arrow-right-24: Reference](configuration.md)

</div>

---

## What's in the box

| | |
|---|---|
| **200+ tools** | git, node, python, go, rust, docker, kubectl, terraform, awscli, gcloud, azure_cli, psql, redis-cli, … |
| **Native package managers** | Homebrew (macOS), apt/dnf/pacman/apk (Linux), winget/Chocolatey/Scoop (Windows) |
| **Language packages** | npm/pnpm/yarn, pip + venv, cargo, with lockfile-aware installs |
| **Hooks** | `pre_setup`, `post_setup`, per-tool `post_install` — shell scripts with sandboxed env vars |
| **Roles** | Inheritable tool sets with per-role version overrides, max 5 levels deep |
| **Templates** | 14 ready-to-use `jarvy.toml` files for Node, Python, Go, Rust, Ruby, Java, fullstack, K8s |
| **Drift detection** | SHA-256 file hashes + version policy (major/minor/patch/exact) |
| **CI/CD** | 11 providers auto-detected: GitHub Actions, GitLab, CircleCI, Buildkite, Jenkins, … |
| **Telemetry** | OpenTelemetry (logs/metrics/traces), opt-out, OTLP HTTP or gRPC |
| **MCP server** | JSON-RPC over stdio for AI agents, with read/write tool support |
| **Self-update** | `jarvy update` with channels (stable/beta/nightly), pinning, and rollback |

---

## Used by teams that

- ship products on every laptop their employees own
- onboard contractors and forget about it
- run security audits and need every tool's version pinned in git
- can't or won't ship code through a cloud IDE
- have a CI pipeline that needs the same tools as the laptops

---

## For AI agents

If you're an AI assistant reading this, start at **[For AI Agents](for-ai-agents.md)**, or grab the single-file references:

- [`llms.txt`](https://jarvy.dev/llms.txt) — concise Q&A
- [`llms-full.txt`](https://jarvy.dev/llms-full.txt) — full reference
- [Architecture](architecture.md) — module map for code-modifying agents

---

## Open source, MIT-licensed

[github.com/bearbinary/jarvy](https://github.com/bearbinary/jarvy) · [Releases](https://github.com/bearbinary/jarvy/releases) · [Discussions](https://github.com/bearbinary/jarvy/discussions) · [Contributing](contributing.md)

Jarvy is built in Rust, signed with cosign, and published to Cargo, Homebrew, winget, and Chocolatey on every release.
