<img src="assets/Vertical_Logo.svg" alt="Jarvy" width="200" />

[![CI](https://github.com/Cliftonz/jarvy/actions/workflows/test.yml/badge.svg)](https://github.com/Cliftonz/jarvy/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/Cliftonz/jarvy/graph/badge.svg)](https://codecov.io/gh/Cliftonz/jarvy)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/Cliftonz/jarvy/badge)](https://securityscorecards.dev/viewer/?uri=github.com/Cliftonz/jarvy)
[![License: MIT](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

# Jarvy

**Jarvy** is a fast, cross-platform CLI that standardizes and automates local development environment setup from a declarative `jarvy.toml` config file. It installs 200+ tools using native package managers (Homebrew, apt, dnf, winget, Chocolatey) and ensures every team member has an identical development environment -- no cloud VMs, no containers, no recurring costs.

## Why Jarvy?

- **Instant onboarding** -- New developers run one command and get a fully configured workstation in seconds, not days
- **Dev environment as code** -- `jarvy.toml` is version-controlled, replacing wiki pages and tribal knowledge
- **Cross-platform** -- Same config works on macOS, Linux, and Windows using native package managers
- **Local and offline** -- No cloud dependency, no container overhead, full native performance
- **Safe and idempotent** -- Run repeatedly; Jarvy detects what's already installed and skips it
- **235+ tools supported** -- From git and docker to terraform, kubectl, and language runtimes
- **Six language ecosystems** -- `[npm]`, `[pip]`, `[cargo]`, `[nuget]`, `[gem]`, `[go]` install language-specific packages alongside CLI tools
- **AI agent integration** -- Distribute guardrails (`[ai_hooks]`), MCP servers (`[mcp_register]`), and AI skills (`[skills]`) across Claude Code / Cursor / Codex / Windsurf / Cline / Continue from one config
- **Git pre-commit hooks** -- `[git_hooks]` installs the framework during `jarvy setup` so teammates ship with the same lint gates
- **Library registry** -- Publish reusable AI hooks, MCP servers, and skills at any HTTPS URL; consumers reference by `use = "name"` (PRD-054)

## Contributor Onboarding (Clean Laptop)

Brand-new machine, nothing installed? Two commands:

```bash
# 1. Install Jarvy
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash

# 2. Clone + provision everything else
git clone https://github.com/Cliftonz/jarvy && cd jarvy && make setup
```

That's it. `make setup` runs [`scripts/bootstrap.sh`](scripts/bootstrap.sh), which is idempotent — re-run any time you've been away from the project or the environment drifts.

**Windows (PowerShell):** swap step 1 for `irm https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.ps1 | iex`, then run the same `git clone ... && cd jarvy && make setup`.

**Already have a tool installed via your own package manager?** Jarvy detects it and skips reinstall. It only adds what's missing.

**Want to skip the curl pipe?** `brew install jarvy` or `cargo install jarvy` work too — assuming you already have brew or cargo on the machine.

## Installation

```bash
# With Cargo
cargo install jarvy

# With Homebrew (macOS/Linux)
brew install jarvy

# Or download a binary from GitHub Releases
# https://github.com/Cliftonz/jarvy/releases
```

Verify installation:

```bash
jarvy --version
```

### Early-Release Channel (Opt-In)

Jarvy ships pre-release tags (`-rc.N`, `-beta.N`) to a separate channel so
you can validate fixes and features before they land in stable. Opt in any
of three ways — all are reversible.

**On install** — set `JARVY_CHANNEL` before running the install script:

```bash
# Unix
JARVY_CHANNEL=beta curl -fsSL \
  https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash

# Windows PowerShell
$env:JARVY_CHANNEL = 'beta'
irm https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.ps1 | iex
```

**Per-update** — pass `--channel`:

```bash
jarvy update --channel beta
```

**Persistent** — set in `~/.jarvy/config.toml`:

```toml
[update]
channel = "beta"   # stable | beta | nightly
```

Channel semantics:

| Channel | Accepts |
|---|---|
| `stable` | Only `vX.Y.Z` tags |
| `beta`   | `vX.Y.Z`, `vX.Y.Z-rc.N`, `vX.Y.Z-beta.N` |
| `nightly` | Everything including `-alpha.N` |

Opting into `beta` is the easiest way to help validate releases. Issues
tagged `release-blocker` or `regression` filed against a pre-release block
its promotion to stable — see
[`docs/release-testing.md`](docs/release-testing.md) for the full process.

## Quick Start

### 1. Create a config

```bash
# Interactive wizard
jarvy init

# Or from a template
jarvy init --template react
jarvy init --template rust-cli
jarvy init --template python-api
jarvy init --template k8s-admin
```

Or create `jarvy.toml` manually in your project root:

```toml
[privileges]
use_sudo = false

[provisioner]
git = "latest"
node = "20"
docker = "latest"
jq = "latest"
```

### 2. Run setup

```bash
jarvy setup
```

Output:

```
Setting up development environment...
  [OK] git 2.44.0 (installed: 2.44.0) - satisfies requirement
  [INSTALL] node 20 - installing via brew...
  [OK] node 20.11.1 installed successfully
  [OK] docker 25.0.3 (installed: 25.0.3) - satisfies requirement
  [INSTALL] jq latest - installing via brew...
  [OK] jq 1.7.1 installed successfully

Setup complete: 4 tools (2 installed, 2 already satisfied)
```

### 3. Share with your team

Commit `jarvy.toml` to your repository. Add to your project's README:

```bash
# Set up development environment
cargo install jarvy && jarvy setup
```

## Configuration

Jarvy supports simple and detailed tool specifications:

```toml
[privileges]
use_sudo = false

[provisioner]
# Simple: tool = "version"
git = "latest"
node = "20"
python = "3.12"
docker = "latest"

# Detailed: tool = { version, version_manager }
rust = { version = "stable", version_manager = true }

# Role-based tool sets
[roles.frontend]
description = "Frontend development"
tools = ["node", "bun", "typescript"]

[roles.devops]
description = "DevOps/Platform engineering"
extends = "base"
tools = ["kubectl", "terraform", "docker", "helm"]

# Post-install hooks
[hooks]
post_setup = "echo 'Environment ready!'"

[hooks.node]
post_install = "corepack enable"

# Environment variables
[env.vars]
PROJECT_ROOT = "$PWD"
NODE_ENV = "development"

# Custom project commands (used by interactive menu)
[commands]
run = "npm start"
test = "npm test"
```

See [Configuration Reference](docs/configuration.md) for all options including environment variables, secrets, services, drift detection, network/proxy settings, git configuration, language packages (`[npm]/[pip]/[cargo]/[nuget]/[gem]/[go]`), AI hooks (`[ai_hooks]`), MCP registration (`[mcp_register]`), git pre-commit hooks (`[git_hooks]`), AI agent skills (`[skills]`), and the library registry (`library_sources`) pattern.

## Shell Completions

Jarvy supports tab completions for all major shells:

```bash
# Bash
jarvy completions bash >> ~/.bashrc

# Zsh
jarvy completions zsh >> ~/.zshrc

# Fish
jarvy completions fish > ~/.config/fish/completions/jarvy.fish

# PowerShell
jarvy completions powershell >> $PROFILE
```

Or view installation instructions for your shell:

```bash
jarvy completions --instructions
```

## Key Commands

| Command | Description |
|---------|-------------|
| `jarvy setup` | Install all tools from `jarvy.toml` |
| `jarvy init` | Create a new config interactively or from a template |
| `jarvy doctor` | Health-check your environment |
| `jarvy validate` | Validate a `jarvy.toml` file |
| `jarvy diff` | Show what's installed vs. what's needed |
| `jarvy drift check` | Detect environment drift from baseline |
| `jarvy tools` | List all 200+ supported tools |
| `jarvy templates list` | Browse available project templates |
| `jarvy completions` | Generate shell completions |
| `jarvy diagnose` | Create a diagnostic bundle for support |
| `jarvy mcp` | Start the MCP server for AI agent integration |
| `jarvy ai-hooks apply` | Distribute AI agent guardrails (Claude Code / Cursor / Codex / Windsurf / Cline / Continue) |
| `jarvy mcp-register apply` | Auto-register Jarvy's MCP server with terminal AI agents so they can discover its tools |

Run `jarvy --help` for the full command reference.

## Use in CI/CD

### GitHub Actions

```yaml
jobs:
  setup:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Jarvy
        uses: Cliftonz/jarvy/.github/actions/setup-jarvy@main
        with:
          method: cargo
      - run: jarvy setup --ci
```

The `--ci` flag enables non-interactive mode with CI-provider detection (GitHub Actions, GitLab CI, CircleCI, Azure DevOps, Jenkins, Bitbucket).

## Scaffold a New Tool

Contributors can add tool support using the built-in scaffolding command:

```bash
# Creates src/tools/mytool/ with the define_tool! macro template
cargo run -p cargo-jarvy -- new-tool mytool

# Or if cargo-jarvy is installed:
cargo jarvy new-tool mytool
```

## AI/LLM Integration

Jarvy includes a built-in [MCP server](docs/mcp-server.md) that lets AI agents discover, check, and install tools via JSON-RPC:

```bash
jarvy mcp
```

For AI agents and LLMs, see [llms.txt](llms.txt) for a structured reference optimized for machine consumption.

## Documentation

- [Quickstart Guide](docs/quickstart.md)
- [Installation](docs/installation.md)
- [Configuration Reference](docs/configuration.md)
- [CLI Reference](docs/cli.md)
- [Hooks](docs/hooks.md)
- [Git pre-commit hooks](docs/git-hooks.md)
- [Adding Tools](docs/adding-tools.md)
- [Language Packages](docs/packages.md)
- [AI Hooks](docs/ai-hooks.md)
- [MCP Registration](docs/mcp-registration.md)
- [AI Agent Skills](docs/skills.md)
- [Library Registry (PRD-054)](docs/library-registry.md)
- [CI/CD Integration](docs/ci-cd.md)
- [MCP Server](docs/mcp-server.md)
- [FAQ](docs/faq.md)
- [Architecture Decisions](docs/decisions.md)
- [Privacy Policy](PRIVACY.md)

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
# Build
cargo build

# Test
cargo test --verbose -- --show-output

# Lint
cargo fmt --all && cargo clippy --all-features -- -D warnings

# Scaffold a new tool
cargo run -p cargo-jarvy -- new-tool <name>
```

## License

Dual-licensed under [MIT](LICENSE) or [Apache-2.0](LICENSE), at your option.

---

[GitHub](https://github.com/Cliftonz/jarvy) | [Documentation](https://jarvy.dev) | [Report an Issue](https://github.com/Cliftonz/jarvy/issues)
