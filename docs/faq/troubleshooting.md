# Troubleshooting FAQ

## I'm getting permission errors during installation

Permission errors typically occur in these scenarios:

### 1. Package Manager Needs Elevated Privileges

Some package managers (apt, dnf) require sudo:

```bash
sudo jarvy setup
```

### 2. Homebrew Ownership Issues (macOS)

If Homebrew directories have incorrect ownership:

```bash
sudo chown -R $(whoami) $(brew --prefix)/*
```

### 3. npm Global Package Permissions

Configure npm to use a user-owned directory:

```bash
mkdir ~/.npm-global
npm config set prefix '~/.npm-global'
echo 'export PATH=~/.npm-global/bin:$PATH' >> ~/.bashrc
source ~/.bashrc
```

### 4. Cargo/Rustup Permissions

Ensure `~/.cargo` is owned by your user:

```bash
sudo chown -R $(whoami) ~/.cargo
```

## How do I diagnose installation issues?

### Quick Diagnostic

```bash
jarvy doctor
```

This checks:
- Package manager availability
- Tool installations and versions
- Dependency satisfaction
- Configuration validity

### Detailed Analysis

```bash
jarvy diagnose
```

Provides comprehensive system information including:
- OS and architecture details
- Package manager versions
- Environment variables
- Network connectivity

### Automatic Fixes

```bash
jarvy doctor --fix
```

Attempts to resolve common issues automatically.

## A tool installed but isn't working

### Check if it's in your PATH

```bash
which <tool-name>
# or
command -v <tool-name>
```

### Restart your shell

Some tools require a new shell session:

```bash
exec $SHELL
# or open a new terminal
```

### Check tool dependencies

Some tools depend on others:

```bash
jarvy doctor
# Look for "Missing dependencies" warnings
```

Common dependency issues:
- `kubectl` needs a Kubernetes cluster (minikube, kind, docker)
- `lazydocker` needs Docker daemon running
- Language tools (kotlin, scala) need their runtime (java)

## Configuration file errors

### Validate your config

```bash
jarvy validate
```

### Common config mistakes

**Invalid version syntax:**
```toml
# Wrong
node = "v20.0.0"

# Correct
node = "20.0.0"
node = "20"
node = "latest"
```

**Missing quotes:**
```toml
# Wrong
git = latest

# Correct
git = "latest"
```

**Invalid TOML structure:**
```toml
# Wrong - can't have both
[provisioner]
node = "20"
node.version = "20"

# Correct - pick one
node = "20"
# OR
[provisioner.node]
version = "20"
```

## Can I use Jarvy offline?

Jarvy requires network connectivity to download packages. For organizations with internal mirrors:

### Homebrew

```bash
export HOMEBREW_BOTTLE_DOMAIN=https://mirrors.example.com/homebrew-bottles
```

### apt

Edit `/etc/apt/sources.list` to point to your internal mirror.

### npm

Create `~/.npmrc`:
```
registry=https://npm.example.com/
```

Fully air-gapped installations are not currently supported.

## Exit codes and their meanings

| Code | Name | Description |
|------|------|-------------|
| 0 | SUCCESS | Operation completed successfully |
| 2 | CONFIG_ERROR | Malformed jarvy.toml |
| 3 | PREREQ_MISSING | Required package manager not found |
| 5 | PERMISSION_REQUIRED | Need elevated privileges (sudo) |

## Getting more help

1. **Check the docs**: https://jarvy.dev/docs
2. **Search existing issues**: https://github.com/jarvy-dev/jarvy/issues
3. **Ask in Discussions**: https://github.com/jarvy-dev/jarvy/discussions
4. **File a bug report**: Use the Bug Report issue template
