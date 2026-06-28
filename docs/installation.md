---
title: "Installation — Jarvy"
description: "Complete installation guide for Jarvy across macOS, Linux, Windows, and from source. For the 60-second version, see Quickstart."
tags:
  - getting-started
---

# Installation

This is the full guide. For the 60-second version, see [Quickstart](quickstart.md).

---

## Requirements

| Platform | Versions | Default package manager |
|----------|----------|-------------------------|
| macOS    | 12 Monterey or newer (Intel + Apple Silicon) | Homebrew |
| Linux    | Ubuntu 22.04+, Debian 12+, Fedora 39+, Arch (rolling), Alpine 3.18+, openSUSE Leap 15.5+ | apt / dnf / pacman / apk / zypper (auto-detected) |
| Windows  | Windows 10 1809+ or Windows 11 | winget (preferred), choco, scoop |
| BSD      | FreeBSD 14+ | pkg |

Jarvy bootstraps the package manager if missing — `jarvy bootstrap` runs the official Homebrew installer on macOS, sets up winget on Windows, etc. You do not need a working package manager before installing Jarvy.

---

## macOS / Linux

=== "One-line installer (recommended)"

    ```bash
    curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash
    ```

    The script:

    1. Detects OS + arch (`x86_64` / `aarch64`)
    2. Downloads the matching release binary from GitHub Releases
    3. Verifies cosign signature (requires `cosign` if `--verify` passed; otherwise skipped with a warning)
    4. Installs to `~/.local/bin/jarvy`
    5. Prints a one-liner to add `~/.local/bin` to `PATH` if needed

    Flags: `--channel <stable|beta|nightly>` (default `stable`), `--version <vX.Y.Z>` (pin specific release), `--prefix <dir>` (alternate install path).

=== "Homebrew (macOS / Linuxbrew)"

    ```bash
    brew install jarvy
    ```

    Tracks the stable channel. `brew upgrade jarvy` to update.

=== "Cargo"

    ```bash
    cargo install jarvy
    ```

    Builds from source — requires Rust 1.85+ (Rust 2024 edition). Slower than the binary installer; useful when no pre-built binary exists for your target.

=== "From source"

    ```bash
    git clone https://github.com/Cliftonz/jarvy.git
    cd jarvy
    cargo build --release
    cp target/release/jarvy ~/.local/bin/
    ```

    Use this when contributing, or when you need a specific commit not yet released.

---

## Windows

=== "PowerShell installer (recommended)"

    ```powershell
    irm https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.ps1 | iex
    ```

    Installs to `%LOCALAPPDATA%\jarvy\bin\jarvy.exe` and adds it to user PATH. Run a fresh PowerShell after install for PATH to take effect.

=== "winget"

    ```powershell
    winget install Cliftonz.Jarvy
    ```

=== "Scoop"

    ```powershell
    scoop bucket add cliftonz https://github.com/Cliftonz/scoop-bucket
    scoop install jarvy
    ```

=== "Chocolatey"

    ```powershell
    choco install jarvy
    ```

=== "Binary"

    Download `jarvy-vX.Y.Z-windows-amd64.zip` from [Releases](https://github.com/Cliftonz/jarvy/releases), extract to a folder, and add that folder to `PATH`.

---

## Verify the install

```bash
jarvy --version          # prints version + commit + build date
jarvy --help             # full command list
jarvy doctor --extended  # checks PATH, package managers, network, write perms
```

If `jarvy: command not found`, `~/.local/bin` (macOS/Linux) or `%LOCALAPPDATA%\jarvy\bin` (Windows) is not in your `PATH`. Re-run the installer or add it manually.

---

## Update

```bash
jarvy update                  # pull latest stable
jarvy update --channel beta   # opt into pre-releases
jarvy update check            # show available without installing
jarvy update --version v0.3.0 # pin specific version
jarvy update --rollback       # restore previous binary (if --keep-backup was used)
```

Update is opt-in by default. Enable background checks with `jarvy update enable`. See [self-update](self-update.md) for the full surface (channels, install-method detection, signature verification).

---

## Uninstall

=== "Installer / from-source"

    ```bash
    rm "$(which jarvy)"
    rm -rf ~/.jarvy        # config, logs, cache, baseline
    ```

=== "Homebrew"

    ```bash
    brew uninstall jarvy
    rm -rf ~/.jarvy
    ```

=== "Cargo"

    ```bash
    cargo uninstall jarvy
    rm -rf ~/.jarvy
    ```

=== "Windows"

    ```powershell
    winget uninstall Cliftonz.Jarvy    # or choco uninstall jarvy / scoop uninstall jarvy
    Remove-Item -Recurse $env:USERPROFILE\.jarvy
    ```

`~/.jarvy` holds your global config, telemetry settings, ticket archives, log rotation. Remove it only when you want a clean slate.

---

## Next

- [Quickstart](quickstart.md) — install + provision in 60 seconds
- [Tutorial: your first jarvy.toml](tutorials/first-config.md) — guided walkthrough
- [Configuration reference](configuration.md) — full `jarvy.toml` schema
- [CLI reference](cli-reference.md) — every command and flag
