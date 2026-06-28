---
title: "Tools by category — Jarvy"
description: "Browse Jarvy's 235+ supported tools grouped by purpose: languages, containers, observability, security, databases, editors, CLI utilities, and more."
tags:
  - tools
  - reference
---

# Tools by category

Jarvy ships first-class support for 235+ developer tools. This page groups them by what they do; the [full registry](tools-registry.md) lists every tool with platform-by-platform package names and version detection details.

To list categories from the CLI:

```bash
jarvy tools --index            # tree view, grouped by category
jarvy search <query>           # fuzzy search across all tools
jarvy tools --default-hooks    # which tools have built-in post-install hooks
```

---

## Languages & runtimes

| Tool | Command | Notes |
|------|---------|-------|
| `node` | node | Installs via official package + npm; use `version_manager = true` to delegate to nvm |
| `bun` | bun | Fast JS runtime + package manager |
| `deno` | deno | Secure JS/TS runtime |
| `python` | python3 | System python by default; pair with pyenv for per-version control |
| `rust` | rustc | Installed via rustup with `default_hook` for PATH |
| `go` | go | Direct binary; honors `GOPATH` / `GOBIN` |
| `ruby` | ruby | System ruby; pair with rbenv for projects |
| `php` | php | With composer for project deps |
| `dotnet` | dotnet | .NET SDK; pair `[nuget]` block for global tools |
| `java` | java | Adoptium Temurin LTS by default |
| `kotlin`, `scala`, `gleam`, `elixir`, `zig`, `julia` | — | One macro per language; see `define_tool!` registry |

---

## Version managers

| Tool | Manages | Notes |
|------|---------|-------|
| `nvm` | Node | Shell-integrated; default hook sources `nvm.sh` |
| `pyenv` | Python | Shell init via default hook |
| `rbenv` | Ruby | Shell init via default hook |
| `mise` | Polyglot (asdf successor) | Recommended over asdf for new projects |
| `asdf` | Polyglot | Legacy; mise is the actively-maintained fork |
| `rustup` | Rust toolchains | Installed transitively with `rust` |

Set `version_manager = true` on the language tool to delegate version selection to its manager: `node = { version = "20", version_manager = true }` installs nvm, then `nvm install 20`.

---

## Containers & Kubernetes

| Tool | Purpose |
|------|---------|
| `docker`, `docker_desktop`, `podman`, `podman_desktop`, `rancher_desktop` | Container runtimes |
| `lazydocker`, `dive`, `ctop` | TUI / inspection |
| `buildah`, `skopeo`, `nerdctl` | Build / image ops without daemon |
| `kubectl`, `helm`, `kustomize` | Kubernetes basics |
| `k9s`, `lazy­kube`, `stern` | TUI / logs |
| `kind`, `minikube`, `k3d`, `talosctl` | Local clusters |
| `kubectx`, `kubens`, `kube-ps1` | Context switching |
| `flux`, `argocd` | GitOps |
| `eksctl`, `aws-iam-authenticator` | AWS EKS |
| `kubescape`, `trivy`, `kube-bench` | Cluster scanning |

---

## DevOps & infrastructure

| Tool | Purpose |
|------|---------|
| `terraform`, `opentofu`, `pulumi`, `cdktf` | IaC |
| `terraform-docs`, `terragrunt`, `tfsec`, `checkov` | Terraform tooling |
| `packer` | Image builds |
| `ansible` | Configuration management |
| `awscli`, `azure_cli`, `gcloud` | Cloud CLIs |
| `tilt`, `skaffold` | Inner-loop dev |
| `vault`, `sops`, `age` | Secrets |
| `act` | Run GitHub Actions locally |

---

## CLI utilities (modern alternatives)

| Tool | Replaces | What you get |
|------|----------|--------------|
| `ripgrep` (`rg`) | grep | Faster, smarter defaults |
| `fd` | find | Sensible defaults, regex |
| `bat` | cat | Syntax highlighting + paging |
| `eza` / `lsd` | ls | Icons, git status, tree view |
| `delta` | diff | Side-by-side, syntax-aware |
| `dust` / `duf` / `ncdu` | du / df | TUI disk usage |
| `procs` / `bottom` / `btop` / `htop` | ps / top | Modern process viewers |
| `zoxide` | cd | Jump to recent dirs |
| `fzf` | — | Fuzzy finder, pipe-anywhere |
| `starship` | PS1 | Async, cross-shell prompt |
| `atuin` | history | SQLite-backed shell history sync |
| `direnv` | .envrc | Per-directory env |
| `jq` / `yq` / `gron` | — | JSON / YAML processing |
| `httpie` / `xh` / `curlie` | curl | Friendlier HTTP CLI |
| `nushell`, `fish` | bash | Structured-data shells |
| `helix`, `nvim`, `micro` | vi | Modal editors |
| `tldr` / `tealdeer` | man | Quick-reference pages |

---

## Databases & data

| Tool | DB |
|------|----|
| `psql`, `pgcli` | Postgres |
| `mysql`, `mycli` | MySQL / MariaDB |
| `sqlite`, `litecli`, `duckdb` | Embedded / analytical |
| `mongosh` | MongoDB |
| `redis` (cli) | Redis |
| `clickhouse-client` | ClickHouse |
| `usql` | Universal SQL CLI |
| `atlas`, `dbmate`, `goose` | Schema migrations |

---

## Observability

| Tool | Purpose |
|------|---------|
| `glances`, `bottom`, `btop`, `htop` | System monitors |
| `mtr`, `nmap`, `dog` | Network |
| `lnav` | Log explorer |
| `stern` | Kubernetes log tail |
| `tempo`, `grafana` (via cookbook) | OTLP backends — see [telemetry-forwarder](operations/telemetry-forwarder.md) |

---

## Security & supply chain

| Tool | Purpose |
|------|---------|
| `gitleaks`, `trufflehog` | Secret scanning |
| `trivy`, `grype`, `syft` | Vulnerability + SBOM |
| `cosign` | Container / artifact signing |
| `kubescape`, `kube-bench` | Kubernetes posture |
| `lynis` | Host hardening audit |
| `semgrep`, `actionlint`, `hadolint`, `shellcheck`, `yamllint`, `ruff`, `shfmt` | Static analysis / linters |

---

## Editors & IDE adjacencies

| Tool | Notes |
|------|-------|
| `vscode`, `cursor` | macOS/Linux via official packages |
| `helix`, `nvim`, `vim`, `emacs`, `micro` | Terminal editors |
| `jetbrains-toolbox` | Manages IntelliJ family installs |
| `iterm2`, `wezterm`, `alacritty` | Terminal emulators |

---

## Git & version control

| Tool | Purpose |
|------|---------|
| `git`, `git-lfs` | Core |
| `gh`, `glab` | GitHub / GitLab CLI |
| `lazygit`, `gitui` | TUIs |
| `delta` | Better diffs |
| `commitizen`, `pre-commit` | Commit hygiene |

---

## Messaging & event infrastructure

| Tool | Purpose |
|------|---------|
| `nats-server`, `kafka` (`kaf`, `kafkactl`), `rabbitmq`, `emqx` | Brokers / clients |
| `redpanda` | Kafka-compatible streaming |
| `kn` | Knative eventing |

---

## Workflow & build

| Tool | Purpose |
|------|---------|
| `make`, `just`, `task` | Task runners |
| `air`, `watchexec`, `entr` | File watchers |
| `hugo`, `zola` | Static site generators |
| `protoc`, `buf` | Protobuf |

---

## File / archive utilities

`p7zip`, `xz`, `rclone`, `aria2`, `wget`, `curl`, `tree`, `fd`, `ncdu` — the workhorses you'd `apt install` blindly.

---

## Adding a tool that's not here

If your favorite tool is missing:

1. Open an issue with the tool name, homepage, and `brew` / `apt` / `winget` package names.
2. Or contribute: see [adding tools](adding-tools.md) — usually 15 lines of `define_tool!` macro plus a `mod.rs` registration.

For the full registry with platform package mappings and version detection notes, see [tools-registry](tools-registry.md).
