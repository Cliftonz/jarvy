---
title: "Contributing - Jarvy"
description: "How to contribute to Jarvy — adding tools, fixing bugs, writing docs, and development setup."
---

# Contributing

Jarvy is open source and welcomes contributions. This guide covers the most common ways to contribute.

## Adding a New Tool

The most common contribution — adding support for a new CLI tool. This requires ~15 lines of code.

### Using the Scaffolder

```bash
cargo run -p cargo-jarvy -- new-tool mytool
```

This creates the directory and boilerplate files.

### Manual Steps

1. Create `src/tools/mytool/mod.rs`:

    ```rust
    mod definition;
    #[allow(unused_imports)]
    pub use definition::*;
    ```

2. Create `src/tools/mytool/definition.rs`:

    ```rust
    //! mytool - brief description
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

3. Add `pub mod mytool;` to `src/tools/mod.rs` (alphabetical order).

4. Run `cargo test --lib` to verify.

### Macro Options

| Option | Example | Description |
|--------|---------|-------------|
| `macos: { brew: "pkg" }` | Homebrew formula | |
| `macos: { cask: "pkg" }` | Homebrew cask (GUI apps) | |
| `linux: { uniform: "pkg" }` | Same name across all distros | |
| `linux: { apt: "x", dnf: "y" }` | Different names per distro | |
| `windows: { winget: "Pub.Pkg" }` | Winget package ID | |
| `windows: { choco: "pkg" }` | Chocolatey package | |
| `custom_install: Some(fn)` | Custom shell script installer | |
| `depends_on: &["docker"]` | Required dependency | |
| `depends_on_one_of: &["a", "b"]` | Flexible dependency | |
| `default_hook: { ... }` | Post-install hook | |

### User-Defined Tools (No Code)

Users can add custom tools without code changes by creating TOML files in `~/.jarvy/tools.d/`:

```toml
name = "my-internal-tool"
command = "my-internal-tool"

[macos]
brew = "my-internal-tool"

[linux]
uniform = "my-internal-tool"
```

## Development Setup

```bash
# Clone and build
git clone https://github.com/bearbinary/jarvy.git
cd jarvy
cargo build

# Or use Jarvy itself
jarvy setup
```

### Build Commands

```bash
cargo build                                    # Debug build
cargo build --release                          # Release build
cargo fmt --all                                # Format code
cargo clippy --all-features -- -D warnings     # Lint
cargo test --verbose -- --show-output          # Run all tests
```

### Running a Single Test

```bash
cargo test --test cli_dispatch -- --show-output
```

### Test Environment Variables

| Variable | Purpose |
|----------|---------|
| `JARVY_TEST_MODE=1` | Disables interactive prompts |
| `JARVY_FAST_TEST=1` | Skips external command execution |

## Code Conventions

- **Rust 2024 edition** idioms
- **Conventional Commits**: `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`
- Prefer stdlib and existing dependencies over new crates
- Run `cargo fmt` and `cargo clippy` before committing
- All public items should have doc comments

## Filing Issues

When filing a bug report, include:

```bash
# Generate a diagnostic bundle
jarvy ticket create
```

This creates a ZIP file with system info, tool versions, and sanitized logs.
