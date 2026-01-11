# Jarvy vs DevPod

A comparison of two approaches to developer environment provisioning.

## Quick Comparison

| Aspect | Jarvy | DevPod |
|--------|-------|--------|
| **Type** | Native CLI tool | Container orchestration platform |
| **Cost Model** | Free, zero ongoing cost | Free software, pay for infrastructure ($20-60/dev/month) |
| **Config Format** | `jarvy.toml` | `devcontainer.json` |
| **Infrastructure** | None (runs locally) | Docker, Kubernetes, or cloud provider |
| **Offline Support** | Full offline capability | Requires container runtime |
| **Interface** | CLI | Desktop GUI app |
| **Language** | Rust | Go |

## When to Choose Jarvy

- **Cost-sensitive teams** - No infrastructure costs, ever
- **Offline-first workflows** - Works without network connectivity
- **Performance-critical work** - Native execution, no container overhead
- **Simple projects** - Lightweight TOML config, minimal setup
- **Resource-constrained machines** - No Docker daemon or VM required
- **Quick onboarding** - Single binary, immediate productivity

## When to Choose DevPod

- **Strict environment isolation** - Full container separation between projects
- **Complex multi-service setups** - Docker Compose-style orchestration
- **Existing devcontainer.json configs** - Direct compatibility with VS Code Dev Containers
- **Remote development needs** - Run environments on remote infrastructure
- **Team-wide standardization** - Identical environments across all developers
- **Windows/Linux parity** - Containers abstract away OS differences

## Key Differentiators

### Jarvy's Approach
- Uses native package managers (Homebrew, apt, etc.) to install tools directly on your system
- No virtualization layer means faster execution and lower resource usage
- Simple mental model: tools are installed where you expect them
- Configuration is intentionally minimal

### DevPod's Approach
- Leverages the devcontainer specification for maximum compatibility
- Provides infrastructure flexibility (local Docker, remote cloud, Kubernetes)
- Offers provider plugins for various cloud platforms
- No vendor lock-in on the software side, but infrastructure costs apply

## Can They Work Together?

Yes. The tools solve different problems and can complement each other:

1. **Use Jarvy for local development** - Fast iteration on your primary projects with native performance
2. **Use DevPod for isolated testing** - Spin up containerized environments when you need strict isolation or to replicate CI conditions
3. **Migration path** - Start with Jarvy for simplicity, adopt DevPod later if container isolation becomes necessary

The choice often comes down to whether you value native performance and zero infrastructure cost (Jarvy) or container isolation and cloud flexibility (DevPod).
