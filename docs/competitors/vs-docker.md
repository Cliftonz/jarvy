# Jarvy vs Docker (Dev Environments)

A comparison of native development environment provisioning vs container-based development.

## Quick Comparison

| Aspect | Jarvy | Docker (Dev Containers) |
|--------|-------|-------------------------|
| **Type** | Native CLI provisioner | Container platform |
| **Cost Model** | Free, MIT licensed | Free (personal) / $5-24/user/month (business) |
| **Config Format** | `jarvy.toml` | `Dockerfile` + `devcontainer.json` |
| **Infrastructure** | None (runs on host) | Docker Engine/Desktop required |
| **Offline Support** | Full (after initial install) | Requires images cached |
| **Performance** | Native execution | Container overhead |
| **Isolation** | None (shared host) | Full container isolation |
| **Resource Usage** | Minimal | Docker daemon + container memory |

## What They Actually Are

### Jarvy
Jarvy is a **development environment provisioner** that installs tools directly on your machine using native package managers (Homebrew, apt, winget). It answers: "What tools do I need installed to work on this project?"

### Docker for Development
Docker provides **containerized development environments** where your code runs inside isolated containers. This includes:
- **Docker Desktop**: GUI app with Docker Engine
- **Dev Containers**: VS Code extension using `devcontainer.json`
- **Docker Compose**: Multi-container development setups

## When to Choose Jarvy

- **Cost-conscious teams** - No Docker Desktop licensing fees
- **Performance-sensitive work** - Native execution without container overhead
- **Simple tool installation** - Just need Node, Python, Go, etc. installed
- **Resource-constrained machines** - No Docker daemon eating 2-4GB RAM
- **Windows ARM or older machines** - Docker compatibility can be problematic
- **Quick project switching** - No container spin-up time
- **Learning/teaching** - Tools installed where students expect them

## When to Choose Docker

- **Strict environment isolation** - Projects can't interfere with each other
- **Linux-specific dependencies** - Need exact Linux versions on macOS/Windows
- **Database/service development** - Postgres, Redis, etc. as containers
- **CI/CD parity** - Identical environment in CI and local
- **Multi-service architectures** - Docker Compose for microservices
- **Team with existing Docker expertise** - Familiar workflow

## Cost Analysis

### Docker Desktop Pricing (2024)

| Tier | Cost | Requirements |
|------|------|--------------|
| Personal | Free | <250 employees, <$10M revenue |
| Pro | $5/user/month | Individual professionals |
| Team | $9/user/month | Team collaboration features |
| Business | $24/user/month | Enterprise security, SSO |

**Team of 25 developers (Business tier)**: $7,200/year for Docker Desktop alone

### Jarvy Pricing

| Tier | Cost |
|------|------|
| All features | $0 |
| Forever | $0 |

## Performance Comparison

| Operation | Jarvy | Docker Dev Container |
|-----------|-------|---------------------|
| Install Node | ~10 seconds | ~30-60 seconds (pull image + setup) |
| Run `node --version` | ~5ms | ~50-100ms (container start) |
| File system access | Native speed | Bind mount overhead (especially macOS) |
| Memory overhead | 0 | 500MB-2GB (daemon + container) |
| Disk usage | Tools only | Images (500MB-5GB per project) |

## Architecture Differences

```
Jarvy:                          Docker:
┌──────────────────┐            ┌──────────────────┐
│   Your Code      │            │   Your Code      │
├──────────────────┤            ├──────────────────┤
│   Node, Python   │◄─ Native   │   Container      │
│   installed on   │   install  │   (Node, Python) │
│   your machine   │            ├──────────────────┤
├──────────────────┤            │   Docker Engine  │
│   macOS/Linux/   │            ├──────────────────┤
│   Windows        │            │   Host OS        │
└──────────────────┘            └──────────────────┘
```

## The Hybrid Approach

Many teams use both tools for different purposes:

```toml
# jarvy.toml - Native tools
[provisioner]
docker = "latest"      # Jarvy installs Docker itself
node = "20"            # Native Node for quick scripts
jq = "latest"
awscli = "latest"

# Then use Docker for services
# docker-compose.yml for Postgres, Redis, etc.
```

**Best of both worlds:**
1. **Jarvy** installs CLI tools natively (fast, simple)
2. **Docker** runs services that benefit from isolation (databases, queues)

## Common Migration Scenarios

### From Docker to Jarvy

**Good candidates:**
- Projects using Docker only for "tool installation"
- Teams frustrated with Docker Desktop performance on macOS
- Cost reduction initiatives
- Simpler projects without complex service dependencies

**Keep Docker if:**
- You need exact Linux kernel features
- Running databases/services in containers
- CI/CD relies on Docker images

### From Jarvy to Docker

**Good candidates:**
- Growing projects needing isolation between environments
- Teams standardizing on devcontainer.json
- Complex multi-service architectures

## Feature Comparison

| Feature | Jarvy | Docker Dev Containers |
|---------|-------|----------------------|
| Cross-platform | macOS, Linux, Windows | macOS, Linux, Windows |
| Version management | Built-in version pinning | Via Dockerfile |
| Service management | Basic (docker-compose start) | Full Docker Compose |
| IDE integration | Editor-agnostic | VS Code, JetBrains |
| Team config sharing | `jarvy.toml` in repo | `devcontainer.json` in repo |
| Hooks/scripts | Post-install hooks | Lifecycle hooks |
| GPU support | Native | Container passthrough |
| Network isolation | None | Full |

## Conclusion

**Choose Jarvy when** you want fast, native tool installation without the complexity and cost of containerization. Ideal for teams that don't need strict isolation.

**Choose Docker when** you need reproducible, isolated environments that match production exactly. Worth the overhead for complex, multi-service applications.

**Use both when** you want native performance for CLI tools but containerized services for databases and dependencies.
