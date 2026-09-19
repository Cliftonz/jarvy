---
title: "Language Packages - Jarvy"
description: "Install npm, pip, cargo, nuget, gem, and go packages alongside CLI tools. Auto-detect package managers, support virtualenvs, install from lockfiles."
---

# Language Packages

`jarvy.toml` can install language-specific packages — `[npm]`, `[pip]`, `[cargo]`, `[nuget]`, `[gem]`, `[go]` — alongside the CLI tools in `[provisioner]`. One config, one command, full project bootstrap.

## Why

A typical project needs both:

- System tools: `node`, `python`, `cargo`, `dotnet`, `ruby`, `go`
- Language packages: `typescript`, `pytest`, `cargo-watch`, `dotnet-ef`, `rubocop`, `golangci-lint`

Without Jarvy, that's two README sections and two onboarding scripts. With Jarvy, it's one `jarvy setup`.

## npm

```toml
[npm]
typescript = "^5.0"
eslint = "latest"
prettier = "^3.0"
package_manager = "pnpm"     # Optional override
from_lockfile = false        # Optional: install from package-lock.json instead
```

### Package Manager Detection

When `package_manager` is omitted, Jarvy auto-detects from lock files in the project root:

| Lock file | Manager |
|-----------|---------|
| `pnpm-lock.yaml` | `pnpm` |
| `yarn.lock` | `yarn` |
| `bun.lockb` | `bun` |
| `package-lock.json` | `npm` |
| (none) | `npm` |

### Lockfile Mode

```toml
[npm]
from_lockfile = true
```

Runs the manager's lockfile install command (`npm ci`, `pnpm install --frozen-lockfile`, `yarn install --frozen-lockfile`). The package list in `[npm]` is ignored — the lockfile is the source of truth.

### Detailed Spec

```toml
[npm]
some-pkg = { version = "^2.0", optional = true }
```

| Field | Type | Description |
|-------|------|-------------|
| `version` | string | Version requirement |
| `optional` | bool | Don't fail if install errors |
| `dev` | bool | Install as devDependency |

## pip

```toml
[pip]
pytest = ">=7.0"
black = "latest"
mypy = "^1.0"
venv = ".venv"               # Path to virtualenv
create_venv = true           # Auto-create if missing
from_lockfile = false        # Use requirements.txt instead
```

### Virtual Environments

When `venv` is set:

1. If the venv directory doesn't exist and `create_venv = true`, Jarvy runs `python -m venv <venv>`
2. Packages install into the venv's `pip`, never global
3. After setup, Jarvy prints an activation hint:

```
Virtualenv ready: source .venv/bin/activate
```

`create_venv = false` and missing venv → install fails fast with a clear message.

### From requirements.txt

```toml
[pip]
from_lockfile = true
```

Runs `pip install -r requirements.txt` (and `requirements-dev.txt` if present). Packages listed in `[pip]` are ignored.

### System Site Packages

```toml
[pip]
system_site_packages = true
```

Passes `--system-site-packages` when creating the venv. Useful for tools that need `apt`-installed Python libs.

## cargo

```toml
[cargo]
cargo-watch = "latest"
cargo-nextest = "0.9"
cargo-deny = "latest"
locked = true                # Pass --locked
```

| Field | Type | Description |
|-------|------|-------------|
| `version` | string | Version requirement |
| `locked` | bool | Use `--locked` for reproducible installs (recommended) |
| `features` | array | Enable specific cargo features |
| `git` | string | Install from git URL instead of crates.io |

### Detailed Spec

```toml
[cargo]
some-tool = { version = "1.0", features = ["feature1", "feature2"] }
git-tool = { git = "https://github.com/owner/repo", locked = true }
```

## nuget (.NET global tools)

```toml
[nuget]
dotnet-ef = "latest"
csharpier = "0.30.0"
dotnet-outdated-tool = "latest"
```

Installs CLI binaries published as NuGet packages via `dotnet tool update -g <name>`. `update` (not `install`) is used so re-runs are idempotent — `install` errors when the tool is already present.

Project-level `<PackageReference>` deps in `.csproj` / `Directory.Packages.props` are NOT managed here; they're restored by `dotnet restore` during build.

### Newer Version Already Installed

```toml
[nuget]
dotnet-ef = { version = "8.0.2", on_newer_installed = "skip" }
```

.NET global tools are not side-by-side: one version per tool, per user. When a newer version is already installed, `dotnet tool update -g <name> --version <pin>` refuses to downgrade, and that package fails. `on_newer_installed` lets a pinned tool opt into softer behavior. It is set per package, on the detailed form only; a plain version string always gets `"strict"`.

| Value | Behavior |
|-------|----------|
| `"strict"` | Default. Always attempt the pinned update; a downgrade conflict fails that package |
| `"skip"` | Check `dotnet tool list -g` first. If the installed version is at or above the pin, print a skip line and leave the tool alone. Never downgrades |
| `"warn"` | Attempt the pinned update. If `dotnet` refuses because the installed version is newer, print a warning, keep the installed tool, and continue. Any other failure still fails that package |

If `dotnet tool list -g` itself fails, Jarvy prints a warning and `"skip"` packages are attempted as `"strict"` for that run.

How `"skip"` compares the installed version to the pin:

- Both sides three-part (`8.0.2`, `1.0.0-alpha.1`): semver ordering, prerelease tags included.
- Either side four-part, and both purely numeric with three or four parts (`8.0.2` vs `8.0.2.1`): numeric comparison, with a missing revision treated as `0`. So `8.0.2` equals `8.0.2.0`, and an installed `8.0.2.1` satisfies a pin of `8.0.2`.
- A four-part numeric version against a three-part semver version: the three-part side is compared by its numeric core, build metadata ignored, and a prerelease always sorts below its own core. So a pin of `8.0.2-rc.1` is satisfied by an installed `8.0.2.0` or `8.0.2.1`; a pin of `8.0.3-rc.1` is not satisfied by an installed `8.0.2.1`.
- Everything else skips only on an exact string match: a four-part version carrying its own prerelease or build suffix, more than four parts, fewer than three parts (such as `8.0`), or a non-version such as `latest`.

`on_newer_installed` is read only under `[nuget]`. Under `[npm]`, `[pip]`, `[cargo]`, `[gem]`, or `[go]`, `jarvy validate` reports it as an error for any value, including `"strict"`, and the JSON schema rejects it in editors; `jarvy setup` ignores the key there, so it has no effect.

Under `[nuget]`, `jarvy validate` also checks the value: anything other than exactly `"strict"`, `"skip"`, or `"warn"` (case-sensitive) is an error naming the allowed values. Left uncaught, a wrong-case value such as `"Skip"` stops `jarvy setup` at config load.

## gem (Ruby)

```toml
[gem]
bundler = "latest"
rubocop = "1.60.0"
solargraph = "latest"
```

Installs via `gem install --no-document <name> [-v <version>]` against the active ruby (system ruby, or whatever rbenv / asdf currently selects). `--no-document` is unconditional — provisioning runs don't need RDoc/RI, and skipping the build cuts install time from ~30s to ~3s on chatty gems.

Bundler workflows (`bundle install` against a project `Gemfile.lock`) are out of scope — run `bundle install` from project bootstrap instead.

## go (Go binaries)

```toml
[go]
"github.com/golangci/golangci-lint/cmd/golangci-lint" = "latest"
"github.com/cosmtrek/air" = "v1.49.0"
"golang.org/x/tools/gopls" = "latest"
```

Installs via `go install <module>@<version>` to the user's `GOBIN` (or `$GOPATH/bin`, or `$HOME/go/bin` fallback). Module paths are full import paths and require quoting in TOML when they contain `/` or `.`. Version is mandatory for Go's tooling outside a `go.mod` tree — use `"latest"` for floating installs.

## Order of Operations

`jarvy setup` runs in this order:

1. CLI tools from `[provisioner]` (parallelized)
2. Per-tool hooks
3. Language packages (`[npm]`, `[pip]`, `[cargo]`, `[nuget]`, `[gem]`, `[go]`)
4. Git config
5. Environment variables
6. Service start (if enabled)
7. Global `post_setup` hook

Language packages run **after** their language runtime is installed, so `node` is always available before `[npm]` runs.

## CLI Behavior

```bash
jarvy setup                          # Includes language packages
jarvy setup --skip-packages          # CLI tools only
jarvy setup --packages-only          # Skip CLI tools
jarvy diff                           # Shows missing packages
jarvy doctor                         # Verifies installed packages
```

## Common Pitfalls

- **No node, but [npm]**: install `node` in `[provisioner]` first, or Jarvy will skip `[npm]` with a warning.
- **pip global pollution**: always set `venv` for project pip deps. Without `venv`, packages install to user-site.
- **cargo-install slow**: `cargo-binstall` is auto-used when present, falling back to `cargo install` otherwise.

## Module

- Source: `src/packages/`
- Files: `config.rs`, `npm.rs`, `pip.rs`, `cargo_pkg.rs`, `nuget.rs`, `gem.rs`, `go.rs`, `common.rs`
- Key types: `PackagesConfig`, `NpmConfig`, `PipConfig`, `CargoConfig`, `NugetConfig`, `GemConfig`, `GoConfig`, `PackageSpec`
