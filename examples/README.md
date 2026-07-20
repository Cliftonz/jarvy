# Jarvy Examples

Drop-in `jarvy.toml` templates for common project types. Pick the one closest to
your stack, copy it to your repo root, edit the tool versions and commands, and
commit.

## How To Use

```bash
# In your project root
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/examples/<template>/jarvy.toml -o jarvy.toml
$EDITOR jarvy.toml
jarvy setup
```

Or if you've cloned this repo locally:

```bash
cp examples/<template>/jarvy.toml /path/to/your/project/
```

## Available Templates

### Language-Specific

| Template | Stack |
|----------|-------|
| [`node-npm/`](node-npm/jarvy.toml) | Node.js + npm |
| [`node-pnpm/`](node-pnpm/jarvy.toml) | Node.js + pnpm |
| [`node-bun/`](node-bun/jarvy.toml) | Bun runtime |
| [`deno/`](deno/jarvy.toml) | Deno |
| [`python-api/`](python-api/jarvy.toml) | Python with pip + venv (FastAPI/Django) |
| [`python-uv/`](python-uv/jarvy.toml) | Python with uv (modern pip replacement) |
| [`go-api/`](go-api/jarvy.toml) | Go HTTP service with air, goose, golangci-lint |
| [`rust-cli/`](rust-cli/jarvy.toml) | Single-crate Rust CLI |
| [`rust-workspace/`](rust-workspace/jarvy.toml) | Multi-crate Cargo workspace |
| [`ruby-rails/`](ruby-rails/jarvy.toml) | Ruby on Rails with Postgres + Redis |
| [`java-spring/`](java-spring/jarvy.toml) | Java + Spring Boot (Maven or Gradle) |
| [`dotnet-api/`](dotnet-api/jarvy.toml) | ASP.NET Core Web API with EF Core |
| [`dotnet-console/`](dotnet-console/jarvy.toml) | .NET console app / CLI / one-shot job |
| [`dotnet-worker/`](dotnet-worker/jarvy.toml) | .NET Worker Service (queue / scheduler / daemon) |
| [`dotnet-grpc/`](dotnet-grpc/jarvy.toml) | ASP.NET Core gRPC service (with grpcurl) |
| [`dotnet-mvc/`](dotnet-mvc/jarvy.toml) | ASP.NET Core MVC web app (Razor views + EF Core) |
| [`dotnet-azure/`](dotnet-azure/jarvy.toml) | ASP.NET Core + Azure cloud-native (`azd`, Bicep, `az`, GitVersion) |
| [`dotnet-microservices/`](dotnet-microservices/jarvy.toml) | Distributed .NET microservices (Dapr + gRPC + SQL Server + Redis) |
| [`nats-services/`](nats-services/jarvy.toml) | NATS-based microservices (broker, CLI, `nsc` auth, gRPC) — language-agnostic |

### Multi-Service

| Template | Stack |
|----------|-------|
| [`react-app/`](react-app/) | React + Vite frontend |
| [`fullstack/`](fullstack/) | Full-stack (frontend + backend + db) |
| [`k8s-platform/`](k8s-platform/) | Platform engineering (kubectl, helm, terraform, ...) |

### Personal Use

| Template | Stack |
|----------|-------|
| [`personal-workstation/`](personal-workstation/jarvy.toml) | Solo-dev laptop bootstrap: shell, editor, CLI upgrades, git identity, runtimes. Live in your dotfiles repo. See [cookbook recipe](../docs/cookbook/personal-workstation.md). |

## Each Template Includes

- `[provisioner]` — language runtime + universal CLI tools
- `[<lang-pkg>]` — language-specific packages (`[npm]`, `[pip]`, `[cargo]`, `[nuget]`)
- `[hooks]` — language-specific post-install setup (lockfile sync, completions, etc.)
- `[env.vars]` — sensible defaults for the language
- `[commands]` — `run` / `test` / `build` / `lint` aliases for `jarvy` interactive menu
- `[drift]` — drift detection enabled with language-appropriate `track_files`

## Customizing

The templates are starting points. Common edits:

- **Pin versions tighter** if you need reproducibility: `node = "20.11.1"` instead of `node = "20"`
- **Add team roles** for monorepos with split frontend/backend: see [`docs/roles.md`](../docs/roles.md)
- **Add proxy config** if you're behind a corporate firewall: see [`docs/network.md`](../docs/network.md)
- **Move secrets out of the file**: use `{ env = "VAR" }` indirection for anything sensitive

## Validate Before Committing

```bash
jarvy validate            # Schema + value check
jarvy diff                # See what would change
jarvy setup --dry-run     # Full plan without execution
```
