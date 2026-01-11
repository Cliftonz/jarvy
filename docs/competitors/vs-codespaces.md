# Jarvy vs GitHub Codespaces

A comparison of Jarvy and GitHub Codespaces for developer environments.

## Quick Comparison

| Feature | Jarvy | GitHub Codespaces |
|---------|-------|-------------------|
| **Type** | Local CLI tool | Cloud-hosted environments |
| **Cost Model** | Free, zero ongoing cost | $0.18-$2.88/hour + storage |
| **Connectivity** | Works offline | Requires internet |
| **Performance** | Native speed | Cloud VM performance |
| **Config Format** | `jarvy.toml` | `devcontainer.json` |
| **Isolation** | None (native install) | Full container isolation |

## When to Choose Jarvy

- **Cost-sensitive teams** - No per-hour charges, ever
- **Offline development** - Work without internet connectivity
- **Native performance** - No virtualization overhead
- **Privacy requirements** - Code never leaves your machine
- **Simple onboarding** - Single config file, familiar local dev
- **Long coding sessions** - No usage-based billing anxiety

## When to Choose Codespaces

- **Ephemeral environments** - Fresh environment per branch/PR
- **GitHub integration** - Deep PR/Actions integration
- **Powerful prebuilds** - Pre-warmed environments ready instantly
- **Hardware flexibility** - Scale up to 32-core machines
- **Browser-based access** - Code from any device
- **Strict isolation** - Container separation between projects

## Cost Comparison

### GitHub Codespaces Pricing
| Machine | Per Hour | 8 hrs/day, 22 days |
|---------|----------|-------------------|
| 2-core | $0.18 | $31.68/month |
| 4-core | $0.36 | $63.36/month |
| 8-core | $0.72 | $126.72/month |
| 16-core | $1.44 | $253.44/month |

Plus storage: $0.07/GB/month

### Team Cost Comparison (Annual)

| Team Size | Jarvy | Codespaces (4-core, moderate use) |
|-----------|-------|-----------------------------------|
| 5 developers | $0 | $3,000 - $4,500 |
| 10 developers | $0 | $6,000 - $9,000 |
| 25 developers | $0 | $15,000 - $22,500 |
| 50 developers | $0 | $30,000 - $45,000 |
| 100 developers | $0 | $60,000 - $90,000 |

## Key Differentiators

### Jarvy Advantages

1. **Zero cost** - No compute charges, no storage fees
2. **Offline capability** - Full functionality without network
3. **Native performance** - No VM/container overhead
4. **No vendor dependency** - Works independent of GitHub
5. **Instant startup** - No environment spin-up time after initial setup

### Codespaces Advantages

1. **Ephemeral workspaces** - Clean slate for every task
2. **Prebuilds** - Environments ready before you need them
3. **Browser access** - Work from any device, anywhere
4. **GitHub integration** - Native PR reviews, Actions
5. **Compute scaling** - Up to 32-core machines available

## Use Case Fit

| Scenario | Better Choice |
|----------|---------------|
| Daily development | Jarvy |
| PR review environments | Codespaces |
| Offline work (travel, poor connectivity) | Jarvy |
| Open source contribution | Codespaces |
| Cost-conscious startups | Jarvy |
| Enterprises with cloud mandates | Codespaces |
| Long coding sessions | Jarvy |
| Quick one-off investigations | Codespaces |

## Summary

**Choose Jarvy** when cost, offline capability, and native performance matter.

**Choose Codespaces** when ephemeral environments, browser access, and GitHub integration are priorities.

For many teams, the $6,000-$90,000 annual Codespaces bill represents the strongest argument for Jarvy's local-first approach.
