# Adding Tools to Jarvy

This guide explains how to add new tools to Jarvy using the declarative ToolSpec pattern.

## Quick Start

For most tools, adding support is just ~15 lines of code:

1. Create a new directory: `src/tools/{toolname}/`
2. Create two files: `mod.rs` and `{toolname}.rs`
3. Add the module to `src/tools/mod.rs`
4. Done! The tool auto-registers at compile time.

## Step-by-Step Guide

### 1. Create the Tool Directory

```bash
mkdir src/tools/mytool
```

### 2. Create mod.rs

Create `src/tools/mytool/mod.rs`:

```rust
#![allow(clippy::module_inception)]
pub mod mytool;
```

### 3. Create the Tool File

Create `src/tools/mytool/mytool.rs`:

```rust
//! mytool - brief description of the tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MYTOOL, {
    command: "mytool",
    macos: { brew: "mytool" },
    linux: { uniform: "mytool" },
    windows: { winget: "Publisher.MyTool" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_mytool_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
```

### 4. Register the Module

Add to `src/tools/mod.rs`:

```rust
pub mod mytool;
```

That's it! The `define_tool!` macro automatically registers the tool via the `inventory` crate.

## The `define_tool!` Macro

The macro handles all boilerplate, generating:
- A `ToolSpec` static with platform-specific install info
- An `ensure()` function that checks/installs the tool
- An `add_handler()` function for the registry
- Auto-registration via `inventory::submit!`

### Macro Syntax

```rust
define_tool!(NAME, {
    command: "command_to_check",   // Required: command to test existence
    macos: { ... },                // Optional: macOS install options
    linux: { ... },                // Optional: Linux install options
    windows: { ... },              // Optional: Windows install options
    custom_install: Some(fn),      // Optional: custom install function
    default_hook: { ... },         // Optional: post-install hook
    depends_on: &[...],            // Optional: strict dependencies (ALL required)
    depends_on_one_of: &[...],     // Optional: flexible dependencies (ONE OF required)
});
```

## Platform-Specific Options

### macOS Options

**Homebrew formula** (CLI tools):
```rust
macos: { brew: "git" },
```

**Homebrew cask** (GUI apps):
```rust
macos: { cask: "docker" },
```

### Linux Options

**Uniform package name** (same name on all distros):
```rust
linux: { uniform: "git" },
```

**Different names per package manager**:
```rust
linux: { apt: "docker.io", dnf: "docker", pacman: "docker", apk: "docker" },
```

**Linuxbrew** (for tools without native packages):
```rust
linux: { brew: "hashicorp/tap/terraform" },
```

### Windows Options

**winget** (preferred):
```rust
windows: { winget: "Git.Git" },
```

**Chocolatey**:
```rust
windows: { choco: "git" },
```

**Both** (winget preferred, choco fallback):
```rust
windows: { winget: "Git.Git", choco: "git" },
```

## Package Name Reference

### Finding Homebrew Packages

```bash
# Search formulas
brew search <name>

# Get package info
brew info <name>

# Search casks (GUI apps)
brew search --cask <name>
```

### Finding Linux Package Names

| Distro | Command |
|--------|---------|
| Debian/Ubuntu | `apt search <name>` |
| Fedora/RHEL | `dnf search <name>` |
| Arch | `pacman -Ss <name>` |
| Alpine | `apk search <name>` |

### Finding winget Package IDs

```powershell
# Search packages
winget search <name>

# The ID is typically "Publisher.PackageName"
# Example: Git.Git, Docker.DockerDesktop, Microsoft.VisualStudioCode
```

## Examples

### Simple Tool (Uniform Package Name)

Tools like `jq` have the same name across all package managers:

```rust
//! jq - command-line JSON processor

use crate::define_tool;

define_tool!(JQ, {
    command: "jq",
    macos: { brew: "jq" },
    linux: { uniform: "jq" },
    windows: { winget: "jqlang.jq" },
});
```

### Tool with Different Package Names

Tools like Docker have different names on different distros:

```rust
//! docker - containerization platform

use crate::define_tool;

define_tool!(DOCKER, {
    command: "docker",
    macos: { cask: "docker" },
    linux: { apt: "docker.io", dnf: "docker", pacman: "docker", apk: "docker" },
    windows: { winget: "Docker.DockerDesktop" },
});
```

### Tool with Homebrew Tap

Some tools require adding a Homebrew tap first. Include the full tap path:

```rust
//! terraform - Infrastructure as Code

use crate::define_tool;

define_tool!(TERRAFORM, {
    command: "terraform",
    macos: { brew: "hashicorp/tap/terraform" },
    linux: { brew: "hashicorp/tap/terraform" },
    windows: { winget: "Hashicorp.Terraform" },
});
```

### macOS-Only Tool

For tools that only work on macOS:

```rust
//! iterm2 - terminal emulator for macOS

use crate::define_tool;

define_tool!(ITERM2, {
    command: "iTerm",
    macos: { cask: "iterm2" },
});
```

## Tool Dependencies

Jarvy supports two types of tool dependencies to ensure proper installation order and prerequisite checking.

### Strict Dependencies (`depends_on`)

Use `depends_on` when a tool **absolutely requires** ALL listed dependencies to function. The tool cannot work without them.

```rust
//! lazydocker - Docker TUI that requires Docker daemon

use crate::define_tool;

define_tool!(LAZYDOCKER, {
    command: "lazydocker",
    macos: { brew: "lazydocker" },
    linux: { brew: "lazydocker" },
    windows: { choco: "lazydocker" },
    // Docker TUI directly calls Docker APIs - Docker is required
    depends_on: &["docker"],
});
```

**When to use `depends_on`:**
- The tool directly calls another tool's APIs (lazydocker → docker)
- The tool is a runtime that runs inside another tool (kind → docker for Kubernetes-in-Docker)
- The tool won't start or function without the dependency

**Behavior:**
- ALL listed dependencies must be available (installed or in config)
- If any dependency is missing from config, a warning is shown
- Dependencies are installed before the dependent tool (topological sort)

### Flexible Dependencies (`depends_on_one_of`)

Use `depends_on_one_of` when a tool needs **at least one** of several alternative dependencies. The user has flexibility in which one to use.

```rust
//! kubectl - Kubernetes CLI that needs a cluster to talk to

use crate::define_tool;

define_tool!(KUBECTL, {
    command: "kubectl",
    macos: { brew: "kubectl" },
    linux: { uniform: "kubectl" },
    windows: { winget: "Kubernetes.kubectl" },
    // kubectl needs ANY K8s cluster provider - user chooses which one
    depends_on_one_of: &["minikube", "kind", "k3d", "docker", "podman"],
});
```

**When to use `depends_on_one_of`:**
- Multiple tools can satisfy the same requirement (container runtimes: docker OR podman)
- The tool works with various backends (kubectl works with any K8s cluster)
- Users should have flexibility in their setup

**Behavior:**
1. If one option is already installed → dependency is satisfied
2. If none installed but one is in config → that one is installed first
3. If none installed or in config → advisory warning (tool still installs)

### Dependency Examples

| Tool | Dependency Type | Dependencies | Reason |
|------|-----------------|--------------|--------|
| `lazydocker` | strict | `["docker"]` | Uses Docker API directly |
| `kind` | strict | `["docker"]` | Runs K8s clusters inside Docker |
| `kubectl` | flexible | `["minikube", "kind", "docker", ...]` | Works with any K8s cluster |
| `minikube` | flexible | `["docker", "podman"]` | Needs a container runtime |
| `helm` | flexible | `["kubectl"]` | Uses kubeconfig, kubectl may be bundled |
| `k9s` | flexible | `["kubectl"]` | K8s TUI, reads kubeconfig |
| `dive` | flexible | `["docker", "podman"]` | Image explorer, works with either |

### Combined Dependencies

A tool can have both strict and flexible dependencies:

```rust
define_tool!(EXAMPLE, {
    command: "example",
    macos: { brew: "example" },
    // Must have git installed
    depends_on: &["git"],
    // And needs one of these container runtimes
    depends_on_one_of: &["docker", "podman"],
});
```

### Dependency Ordering

Jarvy automatically orders tool installation using topological sort:

1. Tools without dependencies are installed first
2. Tools with dependencies are installed after their dependencies
3. For flexible deps, the first matching option in config creates the edge

Example: If config has `[kubectl, minikube, docker]`:
- Order: docker → minikube → kubectl
- (kubectl has flexible dep on minikube; minikube has flexible dep on docker)

### Dependency Check Functions

The spec module provides functions for working with dependencies:

```rust
use jarvy::tools::spec::{
    get_tool_dependencies,           // Get strict deps
    get_tool_flexible_dependencies,  // Get flexible deps
    check_tool_dependencies,         // Check if deps satisfied
    order_tools_by_dependencies,     // Topological sort
};

// Check dependency status
let result = check_tool_dependencies("kubectl", &config_tools, &installed_tools);
match result {
    DependencyCheckResult::Satisfied => { /* good to go */ }
    DependencyCheckResult::MissingRequired(deps) => { /* strict deps missing */ }
    DependencyCheckResult::WillInstallFlexible(dep) => { /* will install this one first */ }
    DependencyCheckResult::MissingFlexible { options, .. } => { /* advisory warning */ }
}
```

## Complex Tools (Custom Installation)

Some tools require custom installation logic that can't be expressed declaratively:
- Shell script installers (nvm, rustup)
- Tools that modify shell configuration
- Tools with complex version management

### When to Use Custom Installation

Use `custom_install` when:
- The tool installs via a shell script (curl | bash pattern)
- The tool requires post-install configuration
- The tool uses a version manager (nvm, pyenv)
- The standard package manager flow doesn't work

### Writing a Custom Installer

For complex tools, don't use `define_tool!`. Instead, implement the handler manually:

```rust
//! nvm - Node Version Manager

use crate::tools::common::{InstallError, has, run};

pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    // Check if already installed
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let ok = std::process::Command::new("bash")
            .args(["-lc", "command -v nvm >/dev/null"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Ok(());
        }
        return install_posix();
    }

    #[cfg(target_os = "windows")]
    {
        if has("nvm") {
            return Ok(());
        }
        return install_windows();
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn install_posix() -> Result<(), InstallError> {
    run("bash", &["-lc",
        "curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash"
    ])?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), InstallError> {
    if !has("winget") {
        return Err(InstallError::Prereq(
            "winget not found. Install Windows Package Manager, then re-run.",
        ));
    }
    run("winget", &["install", "-e", "--id", "CoreyButler.NVMforWindows"])?;
    Ok(())
}

/// Registry adapter
pub fn add_handler(min_hint: &str) -> Result<(), InstallError> {
    ensure(min_hint)
}
```

**Important:** Tools with custom installers must be manually registered in `src/tools/mod.rs`:

```rust
pub fn register_all() {
    // Auto-registered tools via inventory (define_tool! macro)
    for entry in spec::iter_tools() {
        let _ = register_tool(entry.spec.name, entry.handler);
    }

    // Manual registration for custom installers
    let _ = register_tool("nvm", crate::tools::nvm::nvm::add_handler);
}
```

## How Auto-Registration Works

Tools defined with `define_tool!` are automatically registered without any manual code:

1. The `define_tool!` macro generates an `inventory::submit!` call
2. The `inventory` crate collects all submissions at compile time via linker magic
3. `register_all()` iterates over collected entries and registers each tool
4. No manual registration code needed!

```rust
// Inside define_tool! macro expansion:
::inventory::submit! {
    $crate::tools::spec::ToolEntry {
        spec: &MYTOOL,
        handler: add_handler,
    }
}
```

## Testing Your Tool

### Run Tests

```bash
# Run all tests
cargo test --verbose -- --show-output

# Run tests for a specific tool
cargo test mytool -- --show-output
```

### Verify Compilation

```bash
cargo check --verbose
cargo clippy --all-features -- -D warnings
```

### Test Installation (Dry Run)

Create a test `jarvy.toml`:

```toml
[tools]
mytool = "*"
```

Run with test mode:

```bash
JARVY_TEST_MODE=1 cargo run -- setup
```

## Troubleshooting

### Tool Not Found in Registry

1. Check that you added the module to `src/tools/mod.rs`
2. Verify the `define_tool!` macro syntax is correct
3. Run `cargo clean && cargo build` to rebuild

### Package Name Wrong

1. Test the package name manually: `brew install <name>` or `apt install <name>`
2. Use the search commands in the Package Name Reference section
3. Some packages have different names on different distro versions

### Custom Tool Not Registering

Tools with custom installers don't auto-register. Add manual registration:

```rust
// In src/tools/mod.rs register_all()
let _ = register_tool("mytool", crate::tools::mytool::mytool::add_handler);
```

### Compilation Errors

Common issues:
- Missing `use crate::define_tool;` import
- Incorrect brace placement in macro
- Module not declared in parent `mod.rs`

## Using the Scaffolding Command

For convenience, use the scaffolding command to generate boilerplate:

```bash
cargo run -p cargo-jarvy -- new-tool mytool
```

This creates the directory structure and files with sensible defaults.

## How User Requests Reach Maintainers

When a user (or AI agent) runs `jarvy setup` with a tool name that
isn't in the registry — or invokes `jarvy tools --request <name>`
explicitly — Jarvy emits a canonical `tool.unsupported` event. As a
maintainer, this is your primary inbound signal for "add tool X next."

**Two delivery channels:**

1. **Telemetry (preferred)** — when the user has opted in via
   `jarvy telemetry enable` (or implicit consent on the `--request`
   path), the event ships via OTLP to the project's collector. Query
   the `jarvy.tool.unsupported` counter / event stream for ranked
   requests. No GitHub account required for the user, no triage
   friction for you.
2. **GitHub fallback** — when telemetry is off, Jarvy prints a
   pre-filled issue URL pointing at
   [`tool_request.yml`](https://github.com/Cliftonz/jarvy/issues/new?template=tool_request.yml).
   The form arrives with the tool name, platform, version, and
   `define_tool!` scaffold already populated.

**Event payload shape** (uniform across `setup` and `--request`
call sites; see [CLAUDE.md Event Taxonomy](https://github.com/Cliftonz/jarvy/blob/main/CLAUDE.md#telemetry)
for the canonical contract):

| Field | Notes |
|-------|-------|
| `tool` | Unknown tool name |
| `version` | Optional version string from the config |
| `source` | `config` \| `mcp` \| `cli` \| `request` |
| `platform` | `darwin` \| `linux` \| `windows` |
| `suggestions` | CSV of fuzzy-matched candidates |
| `channel` | `telemetry` \| `manual` |
| `fallback_issue_url` | Present only when `channel = manual` |
| `scaffold_cmd` | `cargo run -p cargo-jarvy -- new-tool <name>` |
| `exit_code` | Always `8` (`TOOL_UNSUPPORTED`) |
| `opt_in_bypassed` | `true` when `source = request` |

**Triage workflow:**

1. Rank by `jarvy.tool.unsupported` counter — high-frequency requests
   ship first.
2. Pull `suggestions` to spot typos that aren't real new tools
   (e.g. `dokcer` is just `docker`).
3. Run the suggested `scaffold_cmd` and follow the
   [Quick Start](#quick-start) above. Most additions are ~15 lines
   plus a small test.
4. Tag the PR with `add-tool/<name>` and mention the originating
   issue (or telemetry record ID) so the requesting user gets a
   notification when it ships.

Common asks should ship within days — that's the user-facing promise
the `tool.unsupported` loop is designed to deliver on. See
[`for-ai-agents.md` › When a tool isn't supported](for-ai-agents.md#when-a-tool-isnt-supported)
for the user / AI-agent side of the contract.
