# Jarvy vs Chainguard

A comparison of development environment provisioning vs secure container image distribution.

## Important Distinction

**Jarvy and Chainguard solve different problems in the software lifecycle:**

- **Jarvy**: Development environment setup (installing tools on developer machines)
- **Chainguard**: Secure, minimal container images for production workloads

They're not direct competitors but rather complementary tools for different stages.

## Quick Comparison

| Aspect | Jarvy | Chainguard |
|--------|-------|------------|
| **Primary Use Case** | Developer workstation setup | Production container images |
| **Target Environment** | Local machines | Container runtimes (K8s, Docker) |
| **Cost Model** | Free, MIT licensed | Free tier + Enterprise ($$$) |
| **Security Focus** | Development convenience | CVE-free, hardened images |
| **Output** | Installed tools on host | OCI container images |
| **Config Format** | `jarvy.toml` | Pull from registry |

## What They Actually Are

### Jarvy
A CLI tool that provisions development environments by installing tools (Node, Python, Docker, Terraform, etc.) directly on your machine using native package managers.

```toml
# jarvy.toml
[provisioner]
node = "20"
python = "3.12"
docker = "latest"
```

### Chainguard
A company providing minimal, hardened, CVE-free container base images built on Wolfi (a Linux undistro). Used for running applications in production.

```dockerfile
# Using Chainguard image
FROM cgr.dev/chainguard/python:latest
COPY app.py /app/
CMD ["python", "/app/app.py"]
```

## When They Might Overlap

### Scenario: Building Container Images

A developer needs to build Docker images as part of their workflow:

**With Jarvy:**
```toml
# jarvy.toml - Install Docker and build tools
[provisioner]
docker = "latest"
buildx = "latest"
```

Then in `Dockerfile`:
```dockerfile
# Use Chainguard for minimal, secure base
FROM cgr.dev/chainguard/go:latest AS builder
# ... build steps ...

FROM cgr.dev/chainguard/static:latest
COPY --from=builder /app /app
```

**They work together:** Jarvy installs the tools to build images; Chainguard provides the base images.

## Chainguard's Value Proposition

### The CVE Problem
Traditional base images (Ubuntu, Alpine, Debian) accumulate CVEs:
- Average Docker Hub image: 50-200+ CVEs
- Chainguard images: Near-zero CVEs

### Chainguard Images Features
| Feature | Benefit |
|---------|---------|
| Minimal attack surface | Only what's needed, nothing else |
| Daily rebuilds | Latest security patches |
| SBOM included | Software Bill of Materials for compliance |
| Signed images | Sigstore signatures for verification |
| No shell (distroless) | Harder to exploit |
| Wolfi-based | Purpose-built for containers |

### Pricing

| Tier | Cost | What You Get |
|------|------|--------------|
| Free | $0 | Limited images, `:latest` only |
| Developer | ~$30/month | More images, version tags |
| Enterprise | Contact sales | All images, SLAs, support |

## When to Use Each

### Use Jarvy When

- Setting up a new developer's laptop
- Standardizing CLI tools across a team
- Installing build toolchains (Go, Rust, Node)
- Provisioning CI runner environments
- Quick project onboarding

### Use Chainguard When

- Building production container images
- Meeting security compliance requirements (SOC2, FedRAMP)
- Reducing CVE remediation burden
- Creating minimal, hardened containers
- Supply chain security is critical

## The Full Picture: Dev to Prod

```
Development (Jarvy)          Build (Both)              Production (Chainguard)
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Developer       │     │ CI/CD Pipeline  │     │ Kubernetes      │
│ Workstation     │     │                 │     │                 │
│                 │     │ FROM cgr.dev/   │     │ Secure, minimal │
│ jarvy setup     │────▶│ chainguard/go   │────▶│ container runs  │
│ (installs Go,   │     │ ...             │     │ in production   │
│  Docker, etc.)  │     │ go build        │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

## Combined Workflow Example

### 1. Developer Setup (Jarvy)

```toml
# jarvy.toml
[provisioner]
go = "1.22"
docker = "latest"
kubectl = "latest"
cosign = "latest"    # For verifying Chainguard signatures
```

### 2. Application Development

```go
// main.go
package main
func main() { /* your app */ }
```

### 3. Container Build (Chainguard base)

```dockerfile
# Dockerfile
FROM cgr.dev/chainguard/go:latest AS builder
WORKDIR /src
COPY . .
RUN go build -o /app main.go

FROM cgr.dev/chainguard/static:latest
COPY --from=builder /app /app
ENTRYPOINT ["/app"]
```

### 4. Verify and Deploy

```bash
# Verify Chainguard image signature (cosign installed by Jarvy)
cosign verify cgr.dev/chainguard/static:latest

# Deploy to Kubernetes (kubectl installed by Jarvy)
kubectl apply -f deployment.yaml
```

## Security Considerations

| Aspect | Jarvy | Chainguard |
|--------|-------|------------|
| **Threat Model** | Developer convenience | Production hardening |
| **Attack Surface** | Host OS | Container runtime |
| **CVE Management** | OS package manager updates | Daily rebuilds |
| **Supply Chain** | Trusts package managers | Sigstore signing, SBOM |
| **Isolation** | None (native install) | Container boundaries |

### Jarvy Security Notes
- Installs from official package managers (Homebrew, apt, winget)
- Supports checksum verification for custom installers
- No additional attack surface beyond the tools themselves

### Chainguard Security Notes
- Images rebuilt daily with latest patches
- Signed with Sigstore for provenance
- SBOM available for every image
- Distroless variants have no shell to exploit

## Conclusion

**Jarvy** and **Chainguard** are complementary rather than competing:

- **Use Jarvy** to set up your development environment with the tools you need
- **Use Chainguard** when building secure container images for production

A modern DevSecOps workflow often uses both:
1. Jarvy provisions developer machines and CI runners
2. Chainguard provides the secure base images for production containers

The combination gives you developer productivity (Jarvy) without compromising production security (Chainguard).
