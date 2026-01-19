# Getting Started FAQ

## What is Jarvy?

Jarvy is a cross-platform CLI tool that provisions development environments from a `jarvy.toml` configuration file. It uses native package managers to install tools consistently across your team:

- **macOS**: Homebrew
- **Linux**: apt, dnf, pacman, apk
- **Windows**: winget, Chocolatey

Unlike containerized solutions (Docker, Dev Containers) or VM-based approaches (Vagrant), Jarvy installs tools directly on your machine for native performance and seamless integration with your existing workflow.

## How do I install Jarvy?

### macOS (Homebrew)

```bash
brew install jarvy
```

### Linux

```bash
curl -fsSL https://jarvy.dev/install.sh | bash
```

Or via package managers:

```bash
# Debian/Ubuntu
sudo apt install jarvy

# Fedora
sudo dnf install jarvy

# Arch Linux
yay -S jarvy
```

### Windows

```powershell
# winget
winget install Jarvy.Jarvy

# Chocolatey
choco install jarvy
```

### From Source

```bash
cargo install jarvy
```

## What platforms does Jarvy support?

| Platform | Versions | Architecture |
|----------|----------|--------------|
| macOS | 10.15+ (Catalina and later) | Intel (x86_64), Apple Silicon (arm64) |
| Linux | Ubuntu 20.04+, Debian 11+, Fedora 35+, Arch, Alpine 3.14+ | x86_64, arm64 |
| Windows | Windows 10 1809+, Windows 11 | x86_64 |

Not all 156+ tools are available on all platforms. Use `jarvy tools --platform <os>` to check availability.

## How do I get started?

1. **Install Jarvy** using one of the methods above

2. **Create a `jarvy.toml`** in your project root:

   ```toml
   [provisioner]
   git = "latest"
   node = "20"
   docker = "latest"
   ```

3. **Run setup**:

   ```bash
   jarvy setup
   ```

4. **Verify installation**:

   ```bash
   jarvy doctor
   ```

## What's the difference between Jarvy and...?

See our [competitive analysis](/docs/competitors/) for detailed comparisons with:

- [Homebrew Bundle](/docs/competitors/vs-homebrew-bundle.md)
- [asdf](/docs/competitors/vs-asdf.md)
- [mise](/docs/competitors/vs-mise.md)
- [Nix](/docs/competitors/vs-nix.md)
- [Dev Containers/Codespaces](/docs/competitors/vs-codespaces.md)
