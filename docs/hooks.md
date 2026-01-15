# Hooks

Jarvy hooks allow you to run custom shell scripts at specific points during the setup process. This is useful for:

- Custom tool configuration after installation
- Environment setup and validation
- Post-installation tasks like global package installation
- CI/CD integration and logging

## Hook Types

### Global Hooks

| Hook | When it runs |
|------|--------------|
| `pre_setup` | Once before any tools are installed |
| `post_setup` | Once after all tools have been installed |

### Per-Tool Hooks

| Hook | When it runs |
|------|--------------|
| `post_install` | Immediately after a specific tool is installed |

## Configuration

### Basic Hooks

```toml
[provisioner]
git = "latest"
node = "20"

[hooks]
pre_setup = "echo 'Starting development environment setup...'"
post_setup = "echo 'All tools installed successfully!'"
```

### Per-Tool Hooks

```toml
[provisioner]
git = "latest"
node = "20"
rust = "stable"

[hooks.git]
post_install = "git config --global init.defaultBranch main"

[hooks.node]
post_install = "npm install -g pnpm typescript eslint"

[hooks.rust]
post_install = "rustup component add clippy rustfmt"
```

### Hook Settings

Configure global hook behavior with `[hooks.config]`:

```toml
[hooks.config]
shell = "bash"            # Shell to use for execution
timeout = 300             # Timeout in seconds (default: 300 = 5 minutes)
continue_on_error = false # Whether to continue if a hook fails
```

#### Shell Options

- `bash` - Bash shell (default on most systems)
- `zsh` - Z shell
- `sh` - POSIX shell
- `fish` - Fish shell
- `powershell` - PowerShell (default on Windows)
- `/path/to/shell` - Custom shell path

If not specified, Jarvy auto-detects the shell from the `SHELL` environment variable on Unix, or uses PowerShell on Windows.

## Environment Variables

Hooks have access to the following environment variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `JARVY_TOOL` | Name of the tool being installed (per-tool hooks only) | `node` |
| `JARVY_VERSION` | Version being installed | `20` |
| `JARVY_OS` | Operating system | `macos`, `linux`, `windows` |
| `JARVY_ARCH` | CPU architecture | `x86_64`, `aarch64` |
| `JARVY_HOME` | Jarvy home directory | `/Users/you/.jarvy` |

### Example Using Environment Variables

```toml
[hooks.node]
post_install = """
echo "Installed $JARVY_TOOL version $JARVY_VERSION"
echo "Running on $JARVY_OS ($JARVY_ARCH)"
"""
```

## Multi-line Scripts

Use TOML multi-line strings for complex scripts:

```toml
[hooks]
post_setup = """
#!/bin/bash
echo "Setting up development environment..."

# Configure git
git config --global core.autocrlf input
git config --global pull.rebase true

# Verify installations
echo "Verifying installations:"
git --version
node --version
"""
```

## CLI Flags

### Skip Hooks

Use `--no-hooks` to run setup without executing any hooks:

```sh
jarvy setup --no-hooks
```

### Dry Run

Use `--dry-run` to preview what hooks would run without executing them:

```sh
jarvy setup --dry-run
```

This will show:
- Which hooks would run
- The shell that would be used
- Timeout and continue_on_error settings
- The script content
- Environment variables that would be set

## Error Handling

### Default Behavior

By default, if a hook fails (returns non-zero exit code), the setup process stops and exits with code `7` (HOOK_FAILED).

### Continue on Error

Set `continue_on_error = true` to continue setup even if hooks fail:

```toml
[hooks.config]
continue_on_error = true
```

A warning will be printed for failed hooks, but setup will continue.

### Timeout

Hooks have a default timeout of 300 seconds (5 minutes). If a hook exceeds this timeout, it is terminated and treated as a failure.

```toml
[hooks.config]
timeout = 600  # 10 minutes
```

## Best Practices

1. **Keep hooks idempotent**: Hooks may run multiple times if setup is re-run. Design them to handle this gracefully.

2. **Use descriptive output**: Echo progress messages so users know what's happening.

3. **Handle errors gracefully**: Check for prerequisites and provide helpful error messages.

4. **Test with dry-run first**: Use `jarvy setup --dry-run` to verify hook configuration before running.

5. **Use appropriate timeouts**: Increase timeout for long-running operations like downloading large packages.

## Examples

### Development Environment Setup

```toml
[provisioner]
git = "latest"
node = "20"
python3 = "3.12"
docker = "latest"

[hooks]
pre_setup = "echo 'Starting development environment setup...'"

post_setup = """
echo 'Development environment ready!'
echo 'Installed versions:'
git --version
node --version
python3 --version
docker --version
"""

[hooks.config]
timeout = 600
continue_on_error = false

[hooks.git]
post_install = """
git config --global init.defaultBranch main
git config --global core.autocrlf input
"""

[hooks.node]
post_install = """
npm install -g pnpm
npm install -g typescript
"""

[hooks.python3]
post_install = """
pip3 install --upgrade pip
pip3 install pipenv virtualenv
"""
```

### CI/CD Integration

```toml
[provisioner]
node = "20"
pnpm = "latest"

[hooks]
pre_setup = "echo '::group::Installing development tools'"
post_setup = "echo '::endgroup::'"

[hooks.config]
timeout = 300
continue_on_error = false

[hooks.node]
post_install = "echo 'Node.js $(node --version) installed'"

[hooks.pnpm]
post_install = """
pnpm config set store-dir ~/.pnpm-store
echo 'pnpm $(pnpm --version) configured'
"""
```

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `7` | Hook execution failed (HOOK_FAILED) |

See [Error Codes](./error-codes.md) for the complete list.
