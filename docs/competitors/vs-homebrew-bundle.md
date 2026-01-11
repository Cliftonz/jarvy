# Jarvy vs Homebrew Bundle

A comparison of Jarvy and Homebrew Bundle for developer environment management.

## Quick Comparison

| Feature | Jarvy | Homebrew Bundle |
|---------|-------|-----------------|
| **Purpose** | Cross-platform environment provisioning | macOS package management |
| **Platforms** | macOS, Linux, Windows | macOS (primarily) |
| **Config Format** | `jarvy.toml` | `Brewfile` |
| **Version Pinning** | Explicit versions | Limited (latest by default) |
| **Package Sources** | Homebrew, apt, winget | Homebrew, casks, Mac App Store |
| **Ecosystem** | Built-in tools | Homebrew formulae/casks |

## When to Choose Jarvy

- **Cross-platform teams** - Developers on macOS, Linux, and Windows
- **Explicit versioning** - Pin specific tool versions (`node = "18.16.0"`)
- **Mixed package sources** - Not everything is in Homebrew
- **Team onboarding focus** - Purpose-built for developer setup
- **Simpler syntax** - TOML vs Ruby DSL

## When to Choose Homebrew Bundle

- **macOS-only team** - Everyone uses Macs
- **Mac App Store apps** - Need to install App Store apps
- **Existing Brewfile** - Already have a working Brewfile
- **Homebrew-centric** - All tools available via Homebrew
- **Dump existing setup** - `brew bundle dump` captures current state

## Key Differentiators

### Jarvy's Approach
- Cross-platform first: adapts to each OS's package manager
- Explicit version specifications
- Focus on developer tools specifically
- Simple TOML syntax

### Homebrew Bundle's Approach
- Deep Homebrew integration
- Mac App Store support via `mas`
- Ruby DSL for Brewfile
- Can capture existing system state with `dump`
- Well-established in macOS community

## Configuration Comparison

**Jarvy (jarvy.toml):**
```toml
[tools]
node = "18.16.0"
docker = "latest"
terraform = "1.5.3"
vscode = "latest"
```

**Homebrew Bundle (Brewfile):**
```ruby
brew "node@18"
cask "docker"
brew "terraform"
cask "visual-studio-code"
mas "Xcode", id: 497799835
```

## Can They Work Together?

Yes, on macOS they can complement each other:

1. **Use Jarvy** for core dev tools with version pinning
2. **Use Homebrew Bundle** for Mac-specific apps and App Store installs

However, for most teams, choosing one simplifies maintenance:
- **Jarvy** if you have any non-macOS developers
- **Homebrew Bundle** if you're 100% macOS and need App Store apps

## Feature Comparison

| Feature | Jarvy | Homebrew Bundle |
|---------|-------|-----------------|
| Version pinning | Yes | Limited |
| Windows support | Yes | No |
| Linux support | Yes (apt, etc.) | Limited |
| Mac App Store | No | Yes (via mas) |
| Cask support | Via Homebrew | Native |
| Dump current state | No | Yes |
| Cross-platform config | Single file | macOS only |

## Summary

Jarvy is the better choice for cross-platform teams or when explicit version control matters. Homebrew Bundle is ideal for macOS-only teams who want to leverage the full Homebrew ecosystem including Mac App Store apps.
