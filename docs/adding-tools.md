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
