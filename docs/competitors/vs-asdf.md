# Jarvy vs asdf

A comparison of Jarvy and asdf for developer environment management.

## Quick Comparison

| Feature | Jarvy | asdf |
|---------|-------|------|
| **Purpose** | Full environment provisioning | Runtime version management |
| **Scope** | 40+ tools (runtimes, apps, CLIs) | Language runtimes via plugins |
| **Config Format** | `jarvy.toml` | `.tool-versions` |
| **Mechanism** | Native package managers | Shims + 500+ plugins |
| **Version Switching** | Single version per tool | Multiple versions, per-directory |
| **Language** | Rust | Shell scripts |

## When to Choose Jarvy

- **New machine setup** - Bootstrap a fresh laptop with everything needed
- **Full application installs** - Docker, VS Code, iTerm2, Terraform
- **Infrastructure tools** - AWS CLI, Packer, OpenTofu, k6
- **Team onboarding** - Single command provisions entire environment
- **Cross-platform** - Native support for macOS, Linux, Windows
- **No shim overhead** - Tools installed directly, no command interception

## When to Choose asdf

- **Version switching** - Different Node versions per project
- **Per-directory activation** - Automatic switching on `cd`
- **Plugin ecosystem** - 500+ community plugins for any tool
- **Existing adoption** - Team already uses `.tool-versions` files
- **Shell integration** - Deep integration with bash/zsh
- **Build from source** - Often compiles tools rather than downloading binaries

## Key Differentiators

### Jarvy's Approach
- Uses native package managers (Homebrew, apt, winget)
- Installs full applications, not just runtimes
- Global installation - tools available everywhere
- Simple TOML configuration
- No shims, no shell hooks

### asdf's Approach
- Plugin architecture for any tool
- Shims intercept commands and route to correct version
- Per-directory version files
- Community-maintained plugins
- Shell-based implementation

## Can They Work Together?

Yes. They solve different problems:

1. **Jarvy for machine provisioning** - One-time setup of Docker, VS Code, Terraform, asdf itself
2. **asdf for runtime management** - Per-project Node, Python, Ruby versions

```toml
# jarvy.toml
[tools]
docker = "latest"
vscode = "latest"
terraform = "1.5"
asdf = "latest"  # Jarvy installs asdf
```

```
# .tool-versions (asdf manages these)
nodejs 20.10.0
python 3.12.0
ruby 3.3.0
```

## Migration Considerations

**From asdf to Jarvy:**
- Keep asdf for runtime version management
- Use Jarvy for everything else (Docker, VS Code, CLI tools)
- Jarvy doesn't replace asdf's version switching

**From Jarvy to asdf:**
- asdf won't install Docker, VS Code, or GUI applications
- asdf focuses on runtimes, not full environment provisioning
- Consider using both tools together
