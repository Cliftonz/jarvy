# PRD-023: User Onboarding & Education

## Overview

Add interactive onboarding experiences and educational features that help new users learn Jarvy quickly and effectively, reducing time-to-productivity and support burden.

## Problem Statement

Jarvy currently focuses on technical functionality, but lacks guided experiences for first-time users:
- No interactive quickstart experience
- New users must read documentation to understand basic concepts
- No sample configurations for common development stacks
- No in-CLI help for explaining concepts
- High friction from config file to first successful setup

Users who discover Jarvy need a smooth path from "what is this?" to "I'm using this daily" without requiring extensive documentation reading.

## Evidence

- Developer tools with interactive onboarding (Homebrew's first-run, npm init) see higher adoption
- 70% of CLI tool users prefer learning by doing vs reading docs
- Common first questions: "What should my jarvy.toml look like?", "How do I get started?"
- Competitors with guided setup have lower support burden
- Stack Overflow pattern: users want copy-paste examples, not references

## Requirements

### Functional Requirements

1. **`jarvy init`**: Interactive project initialization wizard
2. **`jarvy quickstart`**: Guided first-run experience
3. **`jarvy templates`**: Browse and use stack templates
4. **First-run detection**: Automatic guidance for new users

### Non-Functional Requirements

1. Interactive prompts work in all terminals (fallback for non-TTY)
2. Quickstart completes in < 3 minutes
3. Templates cover 80% of common development stacks
4. All text is clear, jargon-free, and actionable
5. Works offline after initial template download

## Non-Goals

- Video tutorials or external media
- Web-based learning platform
- Certification or progress tracking
- Multi-language (i18n) support (future PRD)
- IDE/editor integrations

## Command Specifications

### 1. `jarvy init`

Interactive wizard to create a jarvy.toml for the current project.

```bash
# Interactive initialization
jarvy init

# Output:
# Welcome to Jarvy! Let's set up your development environment.
#
# ? What type of project is this?
#   > Web Frontend (React, Vue, Angular)
#     Backend API (Node, Go, Rust, Python)
#     Full Stack (Frontend + Backend)
#     Mobile Development
#     Data Science / ML
#     DevOps / Infrastructure
#     Custom (start from scratch)
#
# ? Which frontend framework? (Use arrow keys)
#   > React
#     Vue
#     Angular
#     Svelte
#     None / Vanilla JS
#
# ? Select additional tools: (Space to select, Enter to confirm)
#   [x] git - Version control
#   [x] node - JavaScript runtime
#   [x] docker - Containers
#   [ ] kubectl - Kubernetes CLI
#   [x] jq - JSON processor
#
# Creating jarvy.toml...
#
# ✓ Created jarvy.toml with 5 tools
#
# Next steps:
#   1. Review your config: cat jarvy.toml
#   2. Install tools: jarvy setup

# Non-interactive mode for CI
jarvy init --template react --non-interactive

# Output to stdout instead of file
jarvy init --stdout

# Specify output path
jarvy init --output path/to/jarvy.toml
```

**Wizard flow:**
1. Detect existing project files (package.json, Cargo.toml, etc.)
2. Suggest appropriate stack based on detection
3. Allow customization of tool selection
4. Generate jarvy.toml with helpful comments
5. Provide clear next steps

### 2. `jarvy quickstart`

Guided first-run experience for new Jarvy users.

```bash
jarvy quickstart

# Output:
# ╔═══════════════════════════════════════════════════════════╗
# ║                    Welcome to Jarvy!                       ║
# ║         Fast, cross-platform developer tool setup          ║
# ╚═══════════════════════════════════════════════════════════╝
#
# Jarvy helps you install and manage developer tools consistently
# across macOS, Linux, and Windows.
#
# Let's get you started in 3 quick steps:
#
# Step 1 of 3: Check your system
# ──────────────────────────────
# ✓ Operating System: macOS 14.2 (supported)
# ✓ Package Manager: Homebrew 4.2.0 (detected)
# ✓ Shell: zsh (completions available)
#
# Step 2 of 3: Create your first config
# ──────────────────────────────────────
# ? Would you like to:
#   > Create a new jarvy.toml (recommended)
#     Import from existing tools (jarvy export)
#     Use a template
#     Skip for now
#
# [User selects "Create a new jarvy.toml"]
# [Runs jarvy init wizard]
#
# Step 3 of 3: Install your tools
# ────────────────────────────────
# Ready to install 5 tools:
#   • git (latest)
#   • node (20)
#   • docker (latest)
#   • jq (latest)
#   • ripgrep (latest)
#
# ? Install now? (Y/n)
#
# [Runs jarvy setup]
#
# 🎉 You're all set!
#
# Useful commands:
#   jarvy search    - Find available tools
#   jarvy upgrade   - Update all tools
#   jarvy --help    - See all commands
#
# Documentation: https://jarvy.dev/docs
```

**Quickstart features:**
- System compatibility check
- Package manager detection
- Guided config creation
- Optional immediate setup
- Next steps and resources

### 3. `jarvy templates`

Browse and use pre-built configuration templates.

```bash
# List available templates
jarvy templates

# Output:
# Available Templates
# ===================
#
# Web Development:
#   react           React + Node.js + common tools (12 tools)
#   vue             Vue.js development stack (10 tools)
#   nextjs          Next.js full-stack template (14 tools)
#   angular         Angular development environment (11 tools)
#   svelte          Svelte/SvelteKit stack (10 tools)
#
# Backend:
#   node-api        Node.js API development (8 tools)
#   go-api          Go backend development (9 tools)
#   rust-cli        Rust CLI development (7 tools)
#   python-api      Python/FastAPI development (11 tools)
#   java-spring     Java Spring Boot development (10 tools)
#
# Data & ML:
#   python-ml       Python ML/Data Science (15 tools)
#   jupyter         Jupyter notebook environment (8 tools)
#
# DevOps:
#   k8s-admin       Kubernetes administration (12 tools)
#   terraform       Infrastructure as Code (9 tools)
#   docker-dev      Docker development (6 tools)
#   cicd            CI/CD pipeline tools (8 tools)
#
# Mobile:
#   flutter         Flutter cross-platform development (9 tools)
#   react-native    React Native mobile development (11 tools)
#
# Minimal:
#   essential       Git + editor + shell tools (5 tools)
#   shell-power     Power shell user toolkit (8 tools)
#
# Use: jarvy templates use <name>

# View template details
jarvy templates show react

# Output:
# Template: react
# ===============
#
# Description: Complete React development environment with modern
#              tooling for building production-ready web applications.
#
# Tools included:
#   • node (20)        - JavaScript runtime
#   • git (latest)     - Version control
#   • docker (latest)  - Containers
#   • jq (latest)      - JSON processor
#   • ripgrep (latest) - Fast search
#   • fd (latest)      - Fast find
#   • bat (latest)     - Better cat
#   • eza (latest)     - Better ls
#   • fzf (latest)     - Fuzzy finder
#   • starship (latest)- Shell prompt
#   • gh (latest)      - GitHub CLI
#   • httpie (latest)  - HTTP client
#
# Hooks:
#   • starship: Shell prompt initialization
#   • nvm: Node version management
#
# Use this template:
#   jarvy templates use react

# Use a template
jarvy templates use react

# Output:
# Using template: react
#
# ? Where to create jarvy.toml?
#   > Current directory (.)
#     Specify path
#
# ✓ Created jarvy.toml from 'react' template
#
# Review and customize:
#   code jarvy.toml   # or your editor
#
# Then install:
#   jarvy setup

# Use template non-interactively
jarvy templates use react --output ./jarvy.toml --non-interactive

# Create from template and install immediately
jarvy templates use react --setup
```

**Template features:**
- Curated templates for common stacks
- Clear tool lists with descriptions
- Customizable after generation
- Version-pinned for stability
- Community-contributed templates (future)

### 4. First-Run Detection

Automatic guidance for new users.

```bash
# When jarvy is run for the first time (any command)
jarvy setup

# Output (first time only):
# ╭──────────────────────────────────────────────────────────╮
# │ 👋 Welcome to Jarvy!                                      │
# │                                                           │
# │ Looks like this is your first time using Jarvy.          │
# │                                                           │
# │ Quick options:                                            │
# │   jarvy quickstart  - Guided setup (recommended)         │
# │   jarvy init        - Create a config file               │
# │   jarvy templates   - Browse starter templates           │
# │                                                           │
# │ To skip this message: jarvy config set show_welcome false │
# ╰──────────────────────────────────────────────────────────╯
#
# [Normal command output follows]
```

**First-run features:**
- Detect first-time users
- Non-intrusive welcome banner
- Quick action suggestions
- Dismissible permanently
- Respects `--quiet` flag
- Does not show in CI environments

## Template Specifications

### Template: react

**Description:** Complete React development environment with modern tooling for building production-ready web applications.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| node | 20 | JavaScript runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| bat | latest | Syntax-highlighted cat |
| eza | latest | Modern ls replacement |
| fzf | latest | Fuzzy finder |
| starship | latest | Cross-shell prompt |
| gh | latest | GitHub CLI |
| httpie | latest | HTTP client |

### Template: vue

**Description:** Vue.js development stack with Vite tooling and essential developer utilities.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| node | 20 | JavaScript runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| bat | latest | Syntax-highlighted cat |
| fzf | latest | Fuzzy finder |
| gh | latest | GitHub CLI |
| httpie | latest | HTTP client |

### Template: nextjs

**Description:** Next.js full-stack template with React, API routes support, and deployment tooling.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| node | 20 | JavaScript runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| bat | latest | Syntax-highlighted cat |
| eza | latest | Modern ls replacement |
| fzf | latest | Fuzzy finder |
| starship | latest | Cross-shell prompt |
| gh | latest | GitHub CLI |
| httpie | latest | HTTP client |
| vercel | latest | Vercel CLI |
| aws | latest | AWS CLI |

### Template: angular

**Description:** Angular development environment with TypeScript tooling and testing utilities.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| node | 20 | JavaScript runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| bat | latest | Syntax-highlighted cat |
| fzf | latest | Fuzzy finder |
| gh | latest | GitHub CLI |
| httpie | latest | HTTP client |
| chromium | latest | Browser testing |

### Template: svelte

**Description:** Svelte/SvelteKit stack with Vite, modern tooling, and optimal DX.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| node | 20 | JavaScript runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| bat | latest | Syntax-highlighted cat |
| fzf | latest | Fuzzy finder |
| gh | latest | GitHub CLI |
| httpie | latest | HTTP client |

### Template: node-api

**Description:** Node.js API development with Express/Fastify patterns and testing tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| node | 20 | JavaScript runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| httpie | latest | HTTP client |
| gh | latest | GitHub CLI |
| redis | latest | Redis CLI |

### Template: go-api

**Description:** Go backend development with common CLI tools and testing utilities.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| go | latest | Go runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| httpie | latest | HTTP client |
| gh | latest | GitHub CLI |
| golangci-lint | latest | Go linter |

### Template: rust-cli

**Description:** Rust CLI development with cargo extensions and debugging tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| rust | latest | Rust toolchain |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| gh | latest | GitHub CLI |

### Template: python-api

**Description:** Python/FastAPI development with virtual environment and testing tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| python | 3.12 | Python runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| httpie | latest | HTTP client |
| gh | latest | GitHub CLI |
| redis | latest | Redis CLI |
| postgresql | latest | PostgreSQL client |
| uv | latest | Fast Python package installer |

### Template: java-spring

**Description:** Java Spring Boot development with Maven/Gradle and debugging tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| java | 21 | Java runtime |
| maven | latest | Maven build tool |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| httpie | latest | HTTP client |
| gh | latest | GitHub CLI |
| redis | latest | Redis CLI |
| postgresql | latest | PostgreSQL client |

### Template: python-ml

**Description:** Python ML/Data Science with Jupyter, scientific computing, and visualization tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| python | 3.12 | Python runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| gh | latest | GitHub CLI |
| aws | latest | AWS CLI |
| postgresql | latest | PostgreSQL client |
| duckdb | latest | Analytical database |
| uv | latest | Fast Python package installer |
| bat | latest | Syntax-highlighted cat |
| fzf | latest | Fuzzy finder |
| httpie | latest | HTTP client |
| sqlite | latest | SQLite CLI |

### Template: jupyter

**Description:** Jupyter notebook environment with essential data tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| python | 3.12 | Python runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| gh | latest | GitHub CLI |
| duckdb | latest | Analytical database |
| uv | latest | Fast Python package installer |
| sqlite | latest | SQLite CLI |

### Template: k8s-admin

**Description:** Kubernetes administration with cluster management and debugging tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| kubectl | latest | Kubernetes CLI |
| helm | latest | Kubernetes package manager |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| k9s | latest | Kubernetes TUI |
| stern | latest | Multi-pod log tailing |
| kubectx | latest | Context switching |
| gh | latest | GitHub CLI |
| aws | latest | AWS CLI |
| yq | latest | YAML processor |

### Template: terraform

**Description:** Infrastructure as Code with Terraform, cloud CLIs, and validation tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| terraform | latest | Infrastructure as Code |
| git | latest | Version control |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| gh | latest | GitHub CLI |
| aws | latest | AWS CLI |
| yq | latest | YAML processor |
| tflint | latest | Terraform linter |
| terragrunt | latest | Terraform wrapper |

### Template: docker-dev

**Description:** Docker development with compose and container debugging tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| docker | latest | Containerization |
| git | latest | Version control |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| dive | latest | Docker image analyzer |
| lazydocker | latest | Docker TUI |

### Template: cicd

**Description:** CI/CD pipeline tools for building and deploying applications.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| git | latest | Version control |
| docker | latest | Containerization |
| gh | latest | GitHub CLI |
| jq | latest | JSON processing |
| yq | latest | YAML processor |
| act | latest | Run GitHub Actions locally |
| aws | latest | AWS CLI |
| trivy | latest | Security scanner |

### Template: flutter

**Description:** Flutter cross-platform development with mobile tooling.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| flutter | latest | Flutter SDK |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| gh | latest | GitHub CLI |
| android-studio | latest | Android development |
| cocoapods | latest | iOS dependency manager |

### Template: react-native

**Description:** React Native mobile development with iOS/Android tooling.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| node | 20 | JavaScript runtime |
| git | latest | Version control |
| docker | latest | Containerization |
| jq | latest | JSON processing |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| gh | latest | GitHub CLI |
| watchman | latest | File watcher |
| android-studio | latest | Android development |
| cocoapods | latest | iOS dependency manager |
| fastlane | latest | Mobile CI/CD |

### Template: essential

**Description:** Minimal toolkit with git, editor support, and essential shell tools.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| git | latest | Version control |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| bat | latest | Syntax-highlighted cat |
| jq | latest | JSON processing |

### Template: shell-power

**Description:** Power shell user toolkit with productivity enhancers.

**Tools:**
| Tool | Version | Purpose |
|------|---------|---------|
| git | latest | Version control |
| ripgrep | latest | Fast code search |
| fd | latest | Fast file finder |
| bat | latest | Syntax-highlighted cat |
| eza | latest | Modern ls replacement |
| fzf | latest | Fuzzy finder |
| starship | latest | Cross-shell prompt |
| zoxide | latest | Smarter cd |

## Acceptance Criteria

### `jarvy init`
- [ ] Detects existing project files (package.json, Cargo.toml, etc.)
- [ ] Offers appropriate stack suggestions based on detection
- [ ] Provides selection UI for tools
- [ ] Generates valid jarvy.toml with comments
- [ ] Shows clear next steps after creation
- [ ] Supports `--template` for non-interactive use
- [ ] Supports `--stdout` to output to terminal
- [ ] Works in non-TTY environments with `--non-interactive`

### `jarvy quickstart`
- [ ] Runs system compatibility check
- [ ] Detects available package managers
- [ ] Guides through config creation or import
- [ ] Offers immediate setup option
- [ ] Shows useful commands and documentation links
- [ ] Completes in under 3 minutes
- [ ] Works without network (except for setup)

### `jarvy templates`
- [ ] Lists all available templates with descriptions
- [ ] Shows detailed tool list for each template
- [ ] Creates valid jarvy.toml from template
- [ ] Allows immediate setup with `--setup`
- [ ] Covers common development stacks (web, backend, ML, DevOps)
- [ ] Templates are curated and tested
- [ ] Non-interactive mode for scripting

### First-Run Detection
- [ ] Detects first-time users reliably
- [ ] Shows welcome banner once
- [ ] Does not interfere with command output
- [ ] Dismissible permanently via config
- [ ] Respects `--quiet` and `--format json` flags
- [ ] Does not show in CI environments

## Technical Approach

### Module Structure

```
src/
  commands/
    init.rs           # jarvy init wizard
    quickstart.rs     # First-run experience
    templates.rs      # Template management
  onboarding/
    mod.rs            # First-run detection
    welcome.rs        # Welcome banner
    detection.rs      # Project type detection
  templates/
    mod.rs            # Template loading
    builtin.rs        # Built-in templates
    schema.rs         # Template schema
data/
  templates/          # Template TOML files
    react.toml
    vue.toml
    go-api.toml
    ...
```

### Init Wizard Implementation

```rust
// src/commands/init.rs
use inquire::{Select, MultiSelect, Text};

pub struct InitOptions {
    pub template: Option<String>,
    pub non_interactive: bool,
    pub output: Option<PathBuf>,
    pub stdout: bool,
}

pub fn run_init(options: InitOptions) -> Result<(), Error> {
    if options.non_interactive {
        return run_init_non_interactive(options);
    }

    // Detect project type
    let detected = detect_project_type()?;

    // Stack selection
    let stack = Select::new("What type of project is this?", STACKS)
        .with_starting_cursor(detected.suggested_index)
        .prompt()?;

    // Tool selection based on stack
    let default_tools = get_stack_tools(stack);
    let selected_tools = MultiSelect::new("Select tools:", available_tools())
        .with_default(&default_tools)
        .prompt()?;

    // Generate config
    let config = generate_config(selected_tools)?;

    // Write output
    write_config(config, options)?;

    print_next_steps();
    Ok(())
}
```

### Template Schema

```toml
# data/templates/react.toml
[template]
name = "react"
description = "Complete React development environment"
category = "Web Development"
tags = ["frontend", "javascript", "react"]

[tools]
node = "20"
git = "latest"
docker = "latest"
jq = "latest"
ripgrep = "latest"
fd = "latest"
bat = "latest"
eza = "latest"
fzf = "latest"
starship = "latest"
gh = "latest"
httpie = "latest"

[hooks.starship]
description = "Initialize starship prompt"

[meta]
author = "Jarvy Team"
version = "1.0.0"
min_jarvy_version = "0.1.0"
```

### First-Run Detection

```rust
// src/onboarding/detection.rs
use std::fs;
use dirs::config_dir;

const FIRST_RUN_MARKER: &str = ".jarvy_initialized";

pub fn is_first_run() -> bool {
    let marker_path = config_dir()
        .map(|p| p.join("jarvy").join(FIRST_RUN_MARKER));

    match marker_path {
        Some(path) => !path.exists(),
        None => true,
    }
}

pub fn mark_initialized() -> Result<(), Error> {
    let marker_path = config_dir()
        .ok_or(Error::NoConfigDir)?
        .join("jarvy")
        .join(FIRST_RUN_MARKER);

    fs::create_dir_all(marker_path.parent().unwrap())?;
    fs::write(marker_path, "")?;
    Ok(())
}
```

## Implementation Steps

1. Create onboarding module structure
2. Implement project type detection
3. Implement `jarvy init` wizard
4. Create template schema and loader
5. Add built-in templates (20 stacks)
6. Implement `jarvy templates` command
7. Implement `jarvy quickstart` flow
8. Add first-run detection and welcome
9. Write unit tests for each component
10. Write integration tests
11. Update documentation and help text

## Dependencies

- `inquire` - Interactive prompts (already in dependencies)
- No new dependencies required

## Effort Estimate

| Task | Effort |
|------|--------|
| Onboarding module structure | 0.5 days |
| Project type detection | 1 day |
| `jarvy init` wizard | 2 days |
| Template schema & loader | 1 day |
| Built-in templates (20) | 2 days |
| `jarvy templates` command | 1 day |
| `jarvy quickstart` flow | 1 day |
| First-run detection | 0.5 days |
| Testing | 2 days |
| Documentation | 1 day |
| **Total** | **12 days** |

## Files to Create/Modify

### New Files
- `src/commands/init.rs`
- `src/commands/quickstart.rs`
- `src/commands/templates.rs`
- `src/onboarding/mod.rs`
- `src/onboarding/welcome.rs`
- `src/onboarding/detection.rs`
- `src/templates/mod.rs`
- `src/templates/builtin.rs`
- `src/templates/schema.rs`
- `data/templates/*.toml` (20 files)
- `tests/init_integration.rs`
- `tests/templates_integration.rs`

### Modified Files
- `src/main.rs` - Add new CLI commands
- `src/commands/mod.rs` - Export new modules
- `Cargo.toml` - No new dependencies needed
- `CLAUDE.md` - Document new commands

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Time to first setup | Unknown (manual) | < 3 minutes |
| Config creation | Manual | Guided wizard |
| Learning method | Read docs | Interactive |
| Template availability | None | 20 stacks |
| First-run guidance | None | Automatic |

## Risks

1. **Interactive prompt compatibility**: Not all terminals support rich prompts
   - Mitigation: Fallback to simple prompts, `--non-interactive` flag

2. **Template maintenance**: Templates become outdated as tools evolve
   - Mitigation: Version templates, automated testing

3. **Project detection accuracy**: May misdetect project types
   - Mitigation: Always allow user override, show what was detected

4. **Overwhelming for experts**: Experienced users may find onboarding annoying
   - Mitigation: One-time welcome, `--quiet` flag, direct command access

---

*PRD-023 v1.2 | User Onboarding & Education | Priority: High*
