# Jarvy vs Gitpod

A comparison of Jarvy and Gitpod for developer environment provisioning.

## Quick Comparison

| Feature | Jarvy | Gitpod |
|---------|-------|--------|
| **Type** | Local CLI tool | Cloud dev environments |
| **Cost Model** | Free / Zero ongoing cost | $25-$50/user/month |
| **Connectivity** | Works offline | Requires internet |
| **Isolation** | Native (uses system package managers) | Container-based |
| **Config Format** | `jarvy.toml` | `devcontainer.json` / `.gitpod.yml` |
| **Performance** | Native speed | Cloud VM performance |
| **Language** | Rust | TypeScript/Go |

## When to Choose Jarvy

- **Cost-sensitive teams** - No per-user monthly fees
- **Offline development** - Work without internet connectivity
- **Native performance requirements** - No virtualization overhead
- **Simple onboarding** - Single config file, familiar local development
- **Privacy-conscious organizations** - All code stays on local machines
- **Resource-constrained environments** - No cloud infrastructure dependencies

## When to Choose Gitpod

- **Ephemeral environments** - Spin up disposable workspaces per branch/PR
- **Multi-Git provider support** - Native GitHub, GitLab, and Bitbucket integration
- **Consistent compute resources** - Standardized cloud VMs for all developers
- **Open source contributions** - Easy onboarding for external contributors
- **Hardware-limited developers** - Offload compute to cloud infrastructure
- **Container-based isolation** - Full environment isolation between projects

## Cost Comparison

| Team Size | Jarvy (Annual) | Gitpod (Annual) |
|-----------|----------------|-----------------|
| 5 developers | $0 | $1,500 - $3,000 |
| 10 developers | $0 | $3,000 - $6,000 |
| 25 developers | $0 | $7,500 - $15,000 |
| 50 developers | $0 | $15,000 - $30,000 |

*Gitpod pricing based on $25-$50/user/month tiers*

## Key Differentiators

### Jarvy Advantages

1. **Zero operational cost** - One-time setup, no recurring fees
2. **Offline-first** - Full functionality without network access
3. **Native tooling** - Uses system package managers (Homebrew, apt, etc.)
4. **No vendor lock-in** - Tools installed directly on your machine
5. **Fast iteration** - No container build or cloud sync delays

### Gitpod Advantages

1. **Ephemeral workspaces** - Fresh environment for every task
2. **Prebuilds** - Pre-warm environments before developers need them
3. **Browser-based access** - Work from any device with a browser
4. **Integrated collaboration** - Share running workspaces with teammates
5. **Multi-provider support** - Works across GitHub, GitLab, and Bitbucket

## Summary

Jarvy is ideal for teams prioritizing cost efficiency, offline capability, and native performance. Gitpod excels when ephemeral cloud environments, browser-based access, and container isolation are priorities.
