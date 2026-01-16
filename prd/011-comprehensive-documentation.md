# PRD-011: Comprehensive Documentation System

## Overview

Create complete, professional documentation for Jarvy covering all implemented features including parallel installation, post-install hooks, environment variables, CI detection, and service management. This PRD supersedes PRD-007 by providing documentation for the full feature set.

## Problem Statement

Jarvy has grown significantly with many advanced features, but documentation hasn't kept pace:

1. **Feature coverage gap**: 9+ PRDs implemented, documentation covers ~20%
2. **No API reference**: `jarvy.toml` schema undocumented
3. **No architecture docs**: Contributors can't understand the system
4. **Missing tool catalog**: 97+ tools with no searchable index
5. **No hook examples**: 29 default hooks exist, none documented
6. **No migration guide**: Users upgrading versions lack guidance

## Current State

### Implemented Features Needing Documentation

| Feature | PRD | Status | Documented |
|---------|-----|--------|------------|
| Parallel tool installation | PRD-001 | Complete | No |
| Batch package manager ops | PRD-001 | Complete | No |
| Tool specification macro | PRD-002 | Complete | Partial |
| Post-install hooks | PRD-003 | Complete | No |
| Default tool hooks (29) | PRD-003.1 | Complete | No |
| Semantic version checking | PRD-004 | Complete | No |
| Environment variables | PRD-008 | Complete | No |
| CI/CD detection (11 providers) | PRD-010 | Complete | No |
| Service management | PRD-009 | Complete | No |
| 97+ tools | Various | Complete | No |

### Existing Documentation

```
docs/
├── Quckstart.md              # Typo, incomplete (9 lines)
├── error-codes.md            # Exists, needs update
└── (nothing else)
```

## Requirements

### Functional Requirements

#### FR-1: User Documentation
1. **Getting Started**
   - Installation guide (all platforms)
   - Quickstart tutorial
   - Migration from v0.x to v1.x

2. **Configuration Reference**
   - Complete `jarvy.toml` schema
   - Version specification syntax
   - Environment variables
   - Hooks configuration
   - Services configuration

3. **CLI Reference**
   - All commands with examples
   - Flags and options
   - Exit codes and error handling

4. **Tool Catalog**
   - Searchable list of 97+ tools
   - Package names per platform
   - Default hooks per tool
   - Version detection notes

#### FR-2: Developer Documentation
1. **Architecture Overview**
   - System diagram
   - Module responsibilities
   - Data flow during setup

2. **Contributing Guide**
   - Adding a new tool
   - Creating default hooks
   - Writing tests
   - Code style guide

3. **API Documentation**
   - Rustdoc for public APIs
   - Internal module docs

#### FR-3: Feature Guides
1. **Hooks Guide**
   - User-defined hooks
   - Default hooks list
   - Hook environment variables
   - Debugging hooks

2. **CI/CD Integration**
   - Supported providers
   - Workflow templates
   - Environment detection

3. **Service Management**
   - Docker Compose integration
   - Tilt integration
   - Auto-start configuration

4. **Environment Variables**
   - `[env]` configuration
   - .env generation
   - Shell rc integration
   - Secrets handling

### Non-Functional Requirements

1. **Discoverability**: Documentation searchable via GitHub and web
2. **Maintainability**: Docs generated from code where possible
3. **Accuracy**: CI validates docs match implementation
4. **Accessibility**: Code examples are copy-paste ready

## Documentation Structure

```
docs/
├── README.md                           # Entry point with navigation
├── getting-started/
│   ├── installation.md                 # Platform-specific install
│   ├── quickstart.md                   # 5-minute tutorial
│   └── migration.md                    # Upgrading versions
├── configuration/
│   ├── jarvy-toml.md                   # Complete schema reference
│   ├── version-syntax.md               # Semver operators
│   ├── hooks.md                        # Hook configuration
│   ├── environment.md                  # [env] section
│   └── services.md                     # [services] section
├── cli/
│   ├── commands.md                     # All commands reference
│   ├── flags.md                        # Global and command flags
│   └── exit-codes.md                   # Error codes (update existing)
├── tools/
│   ├── index.md                        # Searchable tool list
│   ├── by-category.md                  # Tools by type
│   ├── package-mappings.md             # Platform package names
│   └── default-hooks.md                # All 29 default hooks
├── guides/
│   ├── hooks.md                        # Complete hooks guide
│   ├── ci-integration.md               # CI/CD setup
│   ├── services.md                     # Service management
│   ├── environment-setup.md            # Env vars guide
│   └── troubleshooting.md              # FAQ and common issues
├── architecture/
│   ├── overview.md                     # System architecture
│   ├── modules.md                      # Module responsibilities
│   ├── data-flow.md                    # Setup flow diagram
│   └── decisions.md                    # ADRs (Architecture Decision Records)
├── contributing/
│   ├── adding-a-tool.md                # Step-by-step guide
│   ├── creating-hooks.md               # Default hook development
│   ├── testing.md                      # Test patterns
│   └── code-style.md                   # Rust conventions
└── examples/
    ├── node-project.toml
    ├── python-project.toml
    ├── fullstack-project.toml
    ├── devops-toolkit.toml
    ├── rust-development.toml
    └── kubernetes-setup.toml
```

## Document Content Specifications

### 1. Installation Guide

```markdown
# Installation

## Requirements

- macOS 12+, Ubuntu 20.04+, Windows 10+
- Package manager: Homebrew (macOS), apt/dnf (Linux), winget (Windows)

## Quick Install

### macOS / Linux
\`\`\`bash
curl -fsSL https://jarvy.dev/install.sh | bash
\`\`\`

### Windows (PowerShell)
\`\`\`powershell
irm https://jarvy.dev/install.ps1 | iex
\`\`\`

### From Source
\`\`\`bash
cargo install jarvy
\`\`\`

## Verify Installation
\`\`\`bash
jarvy --version
jarvy --help
\`\`\`
```

### 2. Configuration Schema Reference

```markdown
# jarvy.toml Reference

## Complete Schema

\`\`\`toml
# Tool definitions
[tools]
# Simple format
git = "latest"
node = "20"

# Detailed format
[tools.python]
version = "3.12"
use_sudo = false
version_manager = false

# Post-install hooks
[hooks]
pre_setup = "echo 'Starting setup'"
post_setup = "echo 'Setup complete'"

[hooks.node]
post_install = "npm config set prefix ~/.npm-global"

# Environment variables
[env]
NODE_ENV = "development"
DATABASE_URL = "$HOME/.local/db"

[env.secrets]
API_KEY = { prompt = "Enter your API key" }

# Service management
[services]
enabled = true
auto_start = true
compose_file = "docker-compose.yml"

# Global settings
[privileges]
use_sudo = true
```

### 3. Tool Catalog Format

```markdown
# Supported Tools

## Languages & Runtimes

| Tool | Command | Version Check | Default Hook |
|------|---------|---------------|--------------|
| node | `node` | `node --version` | npm global prefix |
| python | `python3` | `python3 --version` | pip user install |
| go | `go` | `go version` | GOPATH setup |
| rust | `rustc` | `rustc --version` | Via rustup |

## DevOps & Infrastructure

| Tool | Command | Default Hook | Notes |
|------|---------|--------------|-------|
| docker | `docker` | Group advisory (Linux) | Desktop on macOS/Win |
| kubectl | `kubectl` | Shell completion | |
| terraform | `terraform` | Shell completion | |
| helm | `helm` | Add bitnami repo | |

## CLI Utilities

| Tool | Command | Default Hook | Notes |
|------|---------|--------------|-------|
| fzf | `fzf` | Shell integration | Fuzzy finder |
| ripgrep | `rg` | Shell completion | Fast grep |
| fd | `fd` | fdfind alias (Debian) | Fast find |
| eza | `eza` | ls/ll/la aliases | Modern ls |
| bat | `bat` | MANPAGER setup | Better cat |
| delta | `delta` | Git pager config | Diff viewer |

<details>
<summary>Package Names by Platform</summary>

| Tool | Homebrew | apt | dnf | pacman | winget |
|------|----------|-----|-----|--------|--------|
| docker | docker | docker.io | docker | docker | Docker.DockerDesktop |
| node | node | nodejs | nodejs | nodejs | OpenJS.NodeJS.LTS |
| python | python | python3 | python3 | python | Python.Python.3 |
| ripgrep | ripgrep | ripgrep | ripgrep | ripgrep | BurntSushi.ripgrep.MSVC |

</details>
```

### 4. Hooks Documentation

```markdown
# Post-Install Hooks

## Overview

Jarvy supports shell scripts that run after tool installation:

- **User-defined hooks**: Configure in `jarvy.toml`
- **Default hooks**: Built into 29 tools (shell completions, aliases, config)

## User-Defined Hooks

\`\`\`toml
[hooks]
# Run before any tools are installed
pre_setup = """
echo "Starting environment setup"
mkdir -p ~/.local/bin
"""

# Run after all tools are installed
post_setup = """
echo "Setup complete!"
source ~/.bashrc
"""

# Per-tool hooks
[hooks.node]
post_install = """
npm config set prefix ~/.npm-global
echo 'export PATH="$HOME/.npm-global/bin:$PATH"' >> ~/.bashrc
"""
\`\`\`

## Hook Environment Variables

| Variable | Description |
|----------|-------------|
| `JARVY_TOOL` | Name of the tool being installed |
| `JARVY_VERSION` | Requested version |
| `JARVY_OS` | Operating system (macos/linux/windows) |
| `JARVY_ARCH` | Architecture (x86_64/aarch64) |
| `JARVY_HOME` | Jarvy config directory |

## Default Hooks

29 tools have built-in hooks:

| Tool | Hook Type | Action |
|------|-----------|--------|
| starship | shell_integration | Add `eval "$(starship init)"` |
| zoxide | shell_integration | Add `eval "$(zoxide init)"` |
| fzf | shell_integration | Configure key bindings |
| direnv | shell_integration | Add `eval "$(direnv hook)"` |
| kubectl | shell_completion | Generate completions |
| helm | config_generation | Add bitnami repository |
| gh | shell_completion | Generate completions |
| delta | config_generation | Configure as git pager |
| eza | shell_alias | Add ls/ll/la/lt aliases |

List all default hooks:
\`\`\`bash
jarvy tools --default-hooks
\`\`\`

## Disabling Hooks

\`\`\`bash
# Skip all hooks
jarvy setup --no-hooks

# Dry run (show what would run)
jarvy setup --dry-run
\`\`\`
```

### 5. CI/CD Integration

```markdown
# CI/CD Integration

## Supported Providers

Jarvy auto-detects 11 CI environments:

| Provider | Detection | Output Format |
|----------|-----------|---------------|
| GitHub Actions | `GITHUB_ACTIONS` | `::group::`, `::error::` |
| GitLab CI | `GITLAB_CI` | `\e[0Ksection_start`, artifacts |
| CircleCI | `CIRCLECI` | Log folding |
| Travis CI | `TRAVIS` | `travis_fold:start` |
| Azure Pipelines | `TF_BUILD` | `##[group]`, `##vso` |
| Jenkins | `JENKINS_URL` | Standard output |
| Bitbucket | `BITBUCKET_BUILD_NUMBER` | Pipe format |
| Buildkite | `BUILDKITE` | `--- :name:` |
| TeamCity | `TEAMCITY_VERSION` | Service messages |
| AppVeyor | `APPVEYOR` | AppVeyor messages |
| Generic CI | `CI=true` | Standard output |

## Usage

\`\`\`bash
# Auto-detect CI environment
jarvy setup

# Force CI mode
jarvy setup --ci

# Disable CI detection
jarvy setup --no-ci

# Check CI info
jarvy ci-info

# Generate workflow template
jarvy ci-config github-actions
\`\`\`

## Workflow Templates

### GitHub Actions

\`\`\`yaml
name: Setup Development Environment
on: [push, pull_request]

jobs:
  setup:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Jarvy
        run: curl -fsSL https://jarvy.dev/install.sh | bash
      - name: Setup Tools
        run: jarvy setup
\`\`\`

### GitLab CI

\`\`\`yaml
setup:
  image: ubuntu:22.04
  script:
    - curl -fsSL https://jarvy.dev/install.sh | bash
    - jarvy setup
\`\`\`
```

### 6. Architecture Overview

```markdown
# Architecture

## System Overview

\`\`\`
┌─────────────────────────────────────────────────────────────┐
│                        jarvy CLI                            │
│  (src/main.rs - clap commands: setup, bootstrap, get, ci)  │
└─────────────────┬───────────────────────────────────────────┘
                  │
    ┌─────────────┼─────────────┬─────────────┐
    ▼             ▼             ▼             ▼
┌────────┐  ┌──────────┐  ┌─────────┐  ┌───────────┐
│ Config │  │  Tools   │  │  Hooks  │  │ Services  │
│        │  │          │  │         │  │           │
│ jarvy  │  │ Registry │  │ Pre/    │  │ Docker    │
│ .toml  │  │ + Specs  │  │ Post    │  │ Compose   │
│ parser │  │ + Batch  │  │ Install │  │ or Tilt   │
└────────┘  └──────────┘  └─────────┘  └───────────┘
                  │
    ┌─────────────┼─────────────┐
    ▼             ▼             ▼
┌────────┐  ┌──────────┐  ┌─────────┐
│ brew   │  │ apt/dnf  │  │ winget  │
│        │  │ pacman   │  │ choco   │
└────────┘  └──────────┘  └─────────┘
\`\`\`

## Module Responsibilities

| Module | Location | Purpose |
|--------|----------|---------|
| CLI | `src/main.rs` | Command parsing, orchestration |
| Config | `src/config.rs` | Parse jarvy.toml |
| Tools | `src/tools/` | Tool definitions, installation |
| Spec | `src/tools/spec.rs` | define_tool! macro, parallel checks |
| Common | `src/tools/common.rs` | Package manager operations |
| Hooks | `src/hooks.rs` | Pre/post install scripts |
| Env | `src/env/` | Environment variable management |
| CI | `src/ci/` | CI provider detection |
| Services | `src/services/` | Docker/Tilt integration |

## Data Flow: Setup Command

\`\`\`
1. Parse jarvy.toml
        │
2. Parallel version check (rayon)
        │
3. Group tools by package manager
        │
4. Run pre_setup hooks
        │
5. Batch install by PM:
   ├── brew install a b c
   ├── apt install -y x y z
   └── custom installers (nvm, rustup)
        │
6. Run post_install hooks (per tool)
        │
7. Run post_setup hooks
        │
8. Start services (if configured)
\`\`\`
```

## Implementation Plan

### Phase 1: Foundation (Week 1)
- [ ] Create docs directory structure
- [ ] Write installation guide
- [ ] Write quickstart tutorial
- [ ] Create jarvy.toml schema reference

### Phase 2: Feature Documentation (Week 2)
- [ ] Document all CLI commands
- [ ] Create hooks guide with examples
- [ ] Write CI/CD integration guide
- [ ] Document environment variables
- [ ] Write services guide

### Phase 3: Tool Catalog (Week 2-3)
- [ ] Generate tool list from codebase
- [ ] Document package name mappings
- [ ] List all 29 default hooks
- [ ] Add tool categories

### Phase 4: Developer Docs (Week 3)
- [ ] Write architecture overview
- [ ] Create contributing guide
- [ ] Document adding a tool
- [ ] Add ADRs for key decisions

### Phase 5: Polish (Week 4)
- [ ] Add example configurations
- [ ] Create troubleshooting FAQ
- [ ] Set up doc search
- [ ] Add CI validation

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Documentation pages | 2 | 25+ |
| Feature coverage | 20% | 100% |
| Example configs | 0 | 6+ |
| Tools documented | 0 | 97+ |
| Hooks documented | 0 | 29 |
| CLI commands documented | 0 | 100% |

## Automation Opportunities

1. **Tool catalog generation**: Script to extract from `define_tool!` macros
2. **Hook list generation**: Extract default hooks from tool files
3. **CLI reference generation**: Generate from clap derive macros
4. **Version sync**: CI check that docs match Cargo.toml version

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Docs drift from code | High | High | Automated generation where possible |
| Incomplete coverage | Medium | Medium | Checklist-driven approach |
| Poor discoverability | Medium | Medium | Good navigation, search |

## Dependencies

- None (pure documentation work)
- Optional: mdBook or similar for generated site

## Effort Estimate

| Task | Effort |
|------|--------|
| Structure and navigation | 0.5 days |
| Installation/quickstart | 1 day |
| Configuration reference | 1 day |
| CLI reference | 0.5 days |
| Hooks guide | 1 day |
| CI/CD guide | 0.5 days |
| Services guide | 0.5 days |
| Tool catalog | 1.5 days |
| Architecture docs | 1 day |
| Contributing guide | 1 day |
| Examples | 0.5 days |
| Review and polish | 1 day |
| **Total** | **~10 days** |

## Files to Create

### New Files
- `docs/README.md`
- `docs/getting-started/installation.md`
- `docs/getting-started/quickstart.md`
- `docs/getting-started/migration.md`
- `docs/configuration/jarvy-toml.md`
- `docs/configuration/version-syntax.md`
- `docs/configuration/hooks.md`
- `docs/configuration/environment.md`
- `docs/configuration/services.md`
- `docs/cli/commands.md`
- `docs/cli/flags.md`
- `docs/tools/index.md`
- `docs/tools/by-category.md`
- `docs/tools/default-hooks.md`
- `docs/guides/hooks.md`
- `docs/guides/ci-integration.md`
- `docs/guides/services.md`
- `docs/guides/environment-setup.md`
- `docs/guides/troubleshooting.md`
- `docs/architecture/overview.md`
- `docs/architecture/modules.md`
- `docs/contributing/adding-a-tool.md`
- `docs/contributing/creating-hooks.md`
- `docs/contributing/testing.md`
- `docs/examples/*.toml`

### Files to Update
- `README.md` - Link to new docs
- `docs/error-codes.md` - Update with new codes
- `CLAUDE.md` - Keep in sync

### Files to Delete
- `docs/Quckstart.md` - Replace with proper quickstart
