# Jarvy vs mise

A comparison of Jarvy and mise (formerly rtx) for developer environment management.

## Quick Comparison

| Feature | Jarvy | mise |
|---------|-------|------|
| **Purpose** | Full environment provisioning | Runtime version management |
| **Scope** | 40+ tools (runtimes, apps, CLIs) | Language runtimes only |
| **Config Format** | `jarvy.toml` | `.mise.toml` / `.tool-versions` |
| **Mechanism** | Native package managers | Shims + plugin system |
| **Version Switching** | Single version per tool | Multiple versions, per-directory |
| **Language** | Rust | Rust |

## When to Choose Jarvy

- **New machine setup** - Bootstrap a fresh laptop with all development tools
- **Full environment provisioning** - Need Docker, VS Code, Terraform, not just runtimes
- **Team onboarding** - Single config file gets new devs productive
- **Cross-platform teams** - macOS, Linux, and Windows support with native installers
- **Simple mental model** - Tools installed globally via familiar package managers

## When to Choose mise

- **Version switching** - Need Node 18 for project A, Node 20 for project B
- **Per-directory versions** - Automatic version switching when `cd`-ing into projects
- **asdf compatibility** - Already using `.tool-versions` files
- **Runtime-only needs** - Only managing language versions, not full applications
- **Plugin ecosystem** - Need support for obscure runtimes via community plugins

## Key Differentiators

### Jarvy's Approach
- Installs tools system-wide using Homebrew, apt, winget
- Provisions complete environments: Docker, VS Code, Terraform, AWS CLI, etc.
- One-time setup per machine
- Tools available everywhere, not just in specific directories

### mise's Approach
- Manages runtime versions via shims that intercept commands
- Per-project version pinning with automatic switching
- Plugin architecture inherited from asdf ecosystem
- Focused exclusively on language runtimes and dev tools

## Can They Work Together?

Yes. A complementary setup:

1. **Jarvy provisions the base** - Docker, VS Code, Terraform, Homebrew, and mise itself
2. **mise manages runtimes** - Node, Python, Ruby versions per project

```toml
# jarvy.toml - provision the machine
[tools]
docker = "latest"
vscode = "latest"
terraform = "1.5"
mise = "latest"  # Install mise via Jarvy
```

```toml
# .mise.toml - manage runtime versions per project
[tools]
node = "20.10.0"
python = "3.12"
```

This gives you the best of both: full environment provisioning with Jarvy, fine-grained version management with mise.
