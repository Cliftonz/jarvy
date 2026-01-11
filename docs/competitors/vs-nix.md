# Jarvy vs Nix/Devenv

A comparison of Jarvy and Nix-based tools for developer environment management.

## Quick Comparison

| Feature | Jarvy | Nix/Devenv |
|---------|-------|------------|
| **Purpose** | Environment provisioning | Reproducible environments |
| **Config Format** | TOML | Nix language / YAML (Devenv) |
| **Learning Curve** | Low | High (Nix), Medium (Devenv) |
| **Isolation** | None (native install) | Sandboxed/hermetic |
| **Rollbacks** | No | Atomic rollbacks |
| **Reproducibility** | Best-effort | Guaranteed |
| **Platform** | macOS, Linux, Windows | Linux, macOS |

## When to Choose Jarvy

- **Quick setup** - Get productive in minutes, not hours
- **Low learning curve** - Familiar TOML config, no new language
- **Windows support** - Full Windows compatibility
- **Team onboarding** - Simple enough for any developer
- **Pragmatic needs** - "Good enough" consistency without complexity
- **Native tooling** - Tools installed where you expect them

## When to Choose Nix/Devenv

- **Reproducibility** - Byte-for-byte identical environments
- **Isolation** - Sandboxed builds that don't pollute system
- **Atomic rollbacks** - Undo any change instantly
- **Complex dependencies** - Projects with intricate dependency graphs
- **CI parity** - Exact same environment in CI and local
- **Functional programming** - Already comfortable with declarative paradigms

## Key Differentiators

### Jarvy's Approach
- Simple TOML configuration anyone can read
- Uses familiar package managers (Homebrew, apt)
- Tools installed globally on the system
- Minutes to learn, works immediately
- Trades reproducibility for simplicity

### Nix's Approach
- Purely functional package manager
- Custom language for configuration
- Hermetic builds with complete isolation
- Massive learning investment required
- Guarantees reproducibility

### Devenv's Approach (Nix simplified)
- Simpler YAML-like configuration on top of Nix
- Reduces Nix learning curve
- Still provides Nix's reproducibility benefits
- Process management (services, databases)
- Still requires Nix installation and concepts

## Can They Work Together?

In theory, yes, but they have different philosophies:

- **Use Jarvy** to bootstrap a machine with essentials (Docker, editors)
- **Use Nix/Devenv** for specific projects requiring strict reproducibility

However, most teams choose one approach:
- **Jarvy** for pragmatic teams prioritizing simplicity and onboarding speed
- **Nix** for teams where reproducibility is non-negotiable

## Learning Curve Comparison

| Task | Jarvy | Nix |
|------|-------|-----|
| Install a tool | 1 line TOML | Learn Nix syntax |
| First environment | 5 minutes | Hours to days |
| Debug issues | Familiar tools | Nix-specific debugging |
| Team adoption | Immediate | Training required |

## Summary

Jarvy is the pragmatic choice: simple, fast, familiar. Nix is the principled choice: reproducible, isolated, powerful. Choose based on whether simplicity or reproducibility is your priority.
