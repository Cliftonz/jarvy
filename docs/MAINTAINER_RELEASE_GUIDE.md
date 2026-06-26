# Jarvy Release & Distribution - Maintainer Guide

This guide documents all credentials, secrets, and steps required to set up and manage Jarvy's release process and package distribution.

## Overview

Jarvy uses automated CI/CD pipelines to build, release, and distribute to multiple package managers. This requires several API keys and credentials to be configured as GitHub repository secrets.

## Distribution Folder Structure

The `dist/` folder contains all packaging configurations for supported platforms:

```
dist/
├── homebrew/
│   └── jarvy.rb              # Homebrew formula for macOS and Linux
├── debian/
│   └── control               # Debian/Ubuntu package metadata
├── rpm/
│   └── jarvy.spec            # RPM spec for Fedora/RHEL/CentOS
├── aur/
│   ├── PKGBUILD              # AUR package (build from source)
│   └── PKGBUILD-bin          # AUR binary package (pre-compiled)
├── windows/
│   ├── winget.yaml           # Windows Package Manager manifest
│   └── chocolatey/
│       ├── jarvy.nuspec      # Chocolatey package spec
│       └── tools/
│           └── chocolateyinstall.ps1  # Chocolatey install script
└── scripts/
    ├── install.sh            # Universal installer for macOS/Linux
    └── install.ps1           # PowerShell installer for Windows
```

### Supported Installation Methods

| Platform | Method | Command |
|----------|--------|---------|
| All | Cargo | `cargo install jarvy` |
| All | GitHub Release | Download from releases page |
| macOS | Homebrew | `brew install Cliftonz/tap/jarvy` |
| macOS/Linux | Curl script | `curl -fsSL https://.../install.sh \| bash` |
| Linux (Arch) | AUR | `yay -S jarvy-bin` |
| Linux (Debian) | .deb package | `dpkg -i jarvy_*.deb` |
| Linux (Fedora) | .rpm package | `rpm -i jarvy-*.rpm` |
| Windows | winget | `winget install Jarvy.Jarvy` |
| Windows | Chocolatey | `choco install jarvy` |
| Windows | PowerShell | `irm .../install.ps1 \| iex` |

## Required GitHub Secrets

The following secrets must be configured in the repository settings under **Settings > Secrets and variables > Actions**:

### 1. crates.io Token (`CRATES_IO_TOKEN`)

**Purpose:** Publish Jarvy to crates.io for `cargo install jarvy`

**How to obtain:**
1. Go to https://crates.io/
2. Log in with your GitHub account
3. Click on your avatar → Account Settings
4. Scroll to "API Tokens"
5. Click "New Token"
6. Name: `jarvy-release`
7. Scopes: Select `publish-new` and `publish-update`
8. Copy the token immediately (it won't be shown again)

**Secret name:** `CRATES_IO_TOKEN`

---

### 2. Homebrew Tap Deploy Key (`HOMEBREW_TAP_DEPLOY_KEY`)

**Purpose:** Push updated formula to the Homebrew tap repository

**Prerequisites:**
1. Create a separate repository: `Cliftonz/homebrew-tap`
2. Add a `jarvy.rb` formula file to this repo

**How to obtain:**
```bash
# Generate SSH key pair
ssh-keygen -t ed25519 -C "jarvy-homebrew-tap" -f jarvy-homebrew-tap

# This creates:
# - jarvy-homebrew-tap (private key)
# - jarvy-homebrew-tap.pub (public key)
```

1. Go to `Cliftonz/homebrew-tap` repository
2. Settings → Deploy keys → Add deploy key
3. Title: `jarvy-release-bot`
4. Key: Paste contents of `jarvy-homebrew-tap.pub`
5. Check "Allow write access"
6. Click "Add key"

**Secret name:** `HOMEBREW_TAP_DEPLOY_KEY`
**Value:** Contents of `jarvy-homebrew-tap` (private key)

---

### 3. AUR Credentials

**Purpose:** Publish to Arch User Repository (AUR)

#### AUR SSH Private Key (`AUR_SSH_PRIVATE_KEY`)

**How to obtain:**
1. Create an AUR account at https://aur.archlinux.org/
2. Generate SSH key:
```bash
ssh-keygen -t ed25519 -C "aur@jarvy" -f aur-jarvy-key
```
3. Add public key to AUR account:
   - Log in to AUR
   - Go to "My Account"
   - Paste contents of `aur-jarvy-key.pub` into SSH Public Key field
   - Click "Update"

**Secret name:** `AUR_SSH_PRIVATE_KEY`
**Value:** Contents of `aur-jarvy-key` (private key)

#### AUR Username (`AUR_USERNAME`)
Your AUR username

#### AUR Email (`AUR_EMAIL`)
Your AUR account email

---

### 4. winget Token (`WINGET_TOKEN`)

**Purpose:** Submit packages to winget-pkgs repository

**How to obtain:**
1. Go to https://github.com/settings/tokens
2. Click "Generate new token (classic)"
3. Name: `jarvy-winget-release`
4. Scopes: Select `public_repo`
5. Generate and copy the token

**Secret name:** `WINGET_TOKEN`

**Note:** The winget submission creates a PR to microsoft/winget-pkgs that requires manual approval.

---

### 5. Chocolatey API Key (`CHOCOLATEY_API_KEY`)

**Purpose:** Push packages to Chocolatey community repository

**How to obtain:**
1. Create an account at https://community.chocolatey.org/
2. Go to your account page
3. Find "API Keys" section
4. Copy your API key

**Secret name:** `CHOCOLATEY_API_KEY`

**Note:** First-time package submissions require manual review. Subsequent updates are usually auto-approved.

---

## Platform-Specific Deployment Processes

This section documents how to update each distribution channel when releasing a new version.

---

### 1. Homebrew (macOS/Linux)

**File:** `dist/homebrew/jarvy.rb`

**Formula Overview:**
The Homebrew formula supports both macOS and Linux, with platform-specific binary URLs:
- macOS Intel: `x86_64-apple-darwin`
- macOS ARM: `aarch64-apple-darwin`
- Linux Intel: `x86_64-unknown-linux-gnu`
- Linux ARM: `aarch64-unknown-linux-gnu`

**Manual Update Process:**
```bash
# 1. Calculate SHA256 checksums for all platform binaries
shasum -a 256 jarvy-v1.0.0-x86_64-apple-darwin.tar.gz
shasum -a 256 jarvy-v1.0.0-aarch64-apple-darwin.tar.gz
shasum -a 256 jarvy-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
shasum -a 256 jarvy-v1.0.0-aarch64-unknown-linux-gnu.tar.gz

# 2. Update dist/homebrew/jarvy.rb:
#    - Replace VERSION_PLACEHOLDER with new version (e.g., 1.0.0)
#    - Replace SHA256_PLACEHOLDER_MACOS_X86 with macOS Intel checksum
#    - Replace SHA256_PLACEHOLDER_MACOS_ARM with macOS ARM checksum
#    - Replace SHA256_PLACEHOLDER_LINUX_X86 with Linux Intel checksum
#    - Replace SHA256_PLACEHOLDER_LINUX_ARM with Linux ARM checksum

# 3. Copy to tap repository
cp dist/homebrew/jarvy.rb ~/homebrew-tap/jarvy.rb

# 4. Test locally
brew install --build-from-source ./jarvy.rb

# 5. Commit and push to tap
cd ~/homebrew-tap
git add jarvy.rb
git commit -m "Update jarvy to v1.0.0"
git push
```

**Users install with:**
```bash
brew tap Cliftonz/tap
brew install jarvy
```

---

### 2. Debian/Ubuntu (.deb packages)

**File:** `dist/debian/control`

**Package Metadata:**
```
Package: jarvy
Version: VERSION_PLACEHOLDER
Section: devel
Priority: optional
Architecture: amd64
Depends: libc6 (>= 2.17)
```

**Manual Build Process:**
```bash
# 1. Create package directory structure
mkdir -p jarvy_1.0.0_amd64/DEBIAN
mkdir -p jarvy_1.0.0_amd64/usr/bin

# 2. Copy control file and update version
cp dist/debian/control jarvy_1.0.0_amd64/DEBIAN/control
sed -i 's/VERSION_PLACEHOLDER/1.0.0/' jarvy_1.0.0_amd64/DEBIAN/control

# 3. Copy binary
cp target/release/jarvy jarvy_1.0.0_amd64/usr/bin/jarvy
chmod 755 jarvy_1.0.0_amd64/usr/bin/jarvy

# 4. Build the package
dpkg-deb --build jarvy_1.0.0_amd64

# 5. Verify package
dpkg-deb --info jarvy_1.0.0_amd64.deb
lintian jarvy_1.0.0_amd64.deb  # Optional: Debian package linter
```

**Users install with:**
```bash
# Download .deb from GitHub releases
sudo dpkg -i jarvy_1.0.0_amd64.deb

# Or with apt (if hosted in repository)
sudo apt install jarvy
```

---

### 3. RPM (Fedora/RHEL/CentOS)

**File:** `dist/rpm/jarvy.spec`

**Spec Overview:**
The RPM spec builds from the pre-compiled binary tarball, not from source.

**Manual Build Process:**
```bash
# 1. Install build tools
sudo dnf install rpm-build rpmdevtools

# 2. Setup RPM build tree
rpmdev-setuptree

# 3. Copy spec file and update version
cp dist/rpm/jarvy.spec ~/rpmbuild/SPECS/
sed -i 's/VERSION_PLACEHOLDER/1.0.0/' ~/rpmbuild/SPECS/jarvy.spec

# 4. Download source tarball to SOURCES
cp jarvy-v1.0.0-x86_64-unknown-linux-gnu.tar.gz ~/rpmbuild/SOURCES/

# 5. Build the RPM
rpmbuild -bb ~/rpmbuild/SPECS/jarvy.spec

# 6. Find the built package
ls ~/rpmbuild/RPMS/x86_64/jarvy-*.rpm
```

**Users install with:**
```bash
# Download .rpm from GitHub releases
sudo rpm -i jarvy-1.0.0-1.x86_64.rpm

# Or with dnf
sudo dnf install jarvy-1.0.0-1.x86_64.rpm
```

---

### 4. AUR (Arch Linux)

**Files:**
- `dist/aur/PKGBUILD` - Build from source
- `dist/aur/PKGBUILD-bin` - Pre-compiled binary (faster)

**Binary Package Update (jarvy-bin):**
```bash
# 1. Clone AUR repository
git clone ssh://aur@aur.archlinux.org/jarvy-bin.git
cd jarvy-bin

# 2. Update PKGBUILD-bin with new version
cp /path/to/dist/aur/PKGBUILD-bin PKGBUILD
sed -i 's/VERSION_PLACEHOLDER/1.0.0/' PKGBUILD

# 3. Update checksums
# Download binaries and calculate:
sha256sum jarvy-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
sha256sum jarvy-v1.0.0-aarch64-unknown-linux-gnu.tar.gz
# Update SHA256_PLACEHOLDER_X86 and SHA256_PLACEHOLDER_ARM in PKGBUILD

# 4. Generate .SRCINFO
makepkg --printsrcinfo > .SRCINFO

# 5. Test build locally
makepkg -si

# 6. Commit and push
git add PKGBUILD .SRCINFO
git commit -m "Update to v1.0.0"
git push
```

**Source Package Update (jarvy):**
```bash
# Same process but using PKGBUILD (builds from source with cargo)
# The sha256sums can be set to 'SKIP' for source tarballs
```

**Users install with:**
```bash
# Binary package (recommended - faster)
yay -S jarvy-bin
# or
paru -S jarvy-bin

# Source package (builds with cargo)
yay -S jarvy
```

---

### 5. Windows Package Manager (winget)

**File:** `dist/windows/winget.yaml`

**Manifest Overview:**
winget uses a YAML manifest submitted to microsoft/winget-pkgs repository.

**Manual Submission Process:**
```powershell
# 1. Update winget.yaml with new version
# Replace VERSION_PLACEHOLDER with version (e.g., 1.0.0)
# Calculate SHA256 of Windows zip file:
Get-FileHash .\jarvy-v1.0.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256
# Replace SHA256_PLACEHOLDER with the hash

# 2. Fork microsoft/winget-pkgs on GitHub

# 3. Create manifest directory
# manifests/j/Jarvy/Jarvy/1.0.0/

# 4. Split manifest into required files (winget v1.6+ format)
# - Jarvy.Jarvy.yaml (version manifest)
# - Jarvy.Jarvy.installer.yaml (installer details)
# - Jarvy.Jarvy.locale.en-US.yaml (localized metadata)

# 5. Submit PR to microsoft/winget-pkgs
# Use wingetcreate for easier submission:
wingetcreate update Jarvy.Jarvy --version 1.0.0 --urls https://github.com/Cliftonz/jarvy/releases/download/v1.0.0/jarvy-v1.0.0-x86_64-pc-windows-msvc.zip --submit
```

**Users install with:**
```powershell
winget install Jarvy.Jarvy
```

**Note:** winget submissions require manual approval from Microsoft maintainers. First-time submissions may take several days.

---

### 6. Chocolatey

**Files:**
- `dist/windows/chocolatey/jarvy.nuspec` - Package specification
- `dist/windows/chocolatey/tools/chocolateyinstall.ps1` - Install script

**Manual Publish Process:**
```powershell
# 1. Update jarvy.nuspec
# Replace VERSION_PLACEHOLDER with version (e.g., 1.0.0)

# 2. Update chocolateyinstall.ps1
# Replace VERSION_PLACEHOLDER with version
# Calculate SHA256:
Get-FileHash .\jarvy-v1.0.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256
# Replace SHA256_PLACEHOLDER with the hash

# 3. Pack the package
cd dist/windows/chocolatey
choco pack jarvy.nuspec

# 4. Test locally
choco install jarvy -source .

# 5. Push to Chocolatey community repository
choco push jarvy.1.0.0.nupkg --api-key $env:CHOCOLATEY_API_KEY
```

**Users install with:**
```powershell
choco install jarvy
```

**Note:** First-time Chocolatey submissions require manual moderation. Subsequent updates are usually auto-approved.

---

### 7. Universal Install Scripts

**Files:**
- `dist/scripts/install.sh` - macOS/Linux installer
- `dist/scripts/install.ps1` - Windows PowerShell installer

These scripts automatically:
1. Detect OS and architecture
2. Fetch the latest release version from GitHub API
3. Download the appropriate binary
4. Install to user's local bin directory
5. Update PATH if needed

**Environment Variables:**
| Variable | Default | Description |
|----------|---------|-------------|
| `JARVY_VERSION` | `latest` | Specific version to install |
| `JARVY_INSTALL_DIR` | `~/.local/bin` (Unix), `%LOCALAPPDATA%\Programs\jarvy` (Win) | Installation directory |
| `JARVY_NO_MODIFY_PATH` | `0` | Set to `1` to skip PATH modification |

**Users install with:**
```bash
# macOS/Linux
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash

# With specific version
JARVY_VERSION=1.0.0 curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash

# Windows PowerShell
irm https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.ps1 | iex
```

**No Updates Required:** Scripts automatically fetch the latest version from GitHub releases. Only update if the script logic needs changes.

---

### 8. crates.io (Cargo)

**File:** `Cargo.toml`

**Publish Process:**
```bash
# Automatically handled by cargo-release or manual:
cargo publish
```

**Users install with:**
```bash
cargo install jarvy
```

---

## One-Time Setup Steps

### 1. Create Homebrew Tap Repository

```bash
# Create repository
gh repo create Cliftonz/homebrew-tap --public

# Clone and initialize
git clone git@github.com:Cliftonz/homebrew-tap.git
cd homebrew-tap

# Copy initial formula
cp /path/to/jarvy/dist/homebrew/jarvy.rb .

# Commit
git add jarvy.rb
git commit -m "Initial formula"
git push
```

Users can then install with:
```bash
brew tap Cliftonz/tap
brew install jarvy
```

### 2. Create AUR Package

```bash
# Clone AUR package (first time creates it)
git clone ssh://aur@aur.archlinux.org/jarvy-bin.git
cd jarvy-bin

# Copy PKGBUILD
cp /path/to/jarvy/dist/aur/PKGBUILD-bin PKGBUILD

# Update version and checksums
# Then commit and push
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO
git commit -m "Initial package"
git push
```

### 3. Install Release Tools Locally

```bash
# cargo-release for version management
cargo install cargo-release

# git-cliff for changelog generation
cargo install git-cliff
```

---

## Release Process

### Standard Release

1. **Prepare the release:**
```bash
# Ensure you're on main with clean working directory
git checkout main
git pull
cargo test
cargo clippy
```

2. **Create the release:**
```bash
# For patch release (0.1.0 -> 0.1.1)
cargo release patch --execute

# For minor release (0.1.0 -> 0.2.0)
cargo release minor --execute

# For major release (0.1.0 -> 1.0.0)
cargo release major --execute
```

This will:
- Bump version in Cargo.toml
- Generate CHANGELOG.md using git-cliff
- Create git commit
- Create git tag (v1.2.3)
- Push to GitHub

3. **Monitor the release:**
- Go to GitHub Actions
- Watch the "Build and Release" workflow
- Once complete, edit the draft release and publish

4. **Verify package updates:**
- Check if Homebrew formula PR is created
- Check if crates.io is updated
- Check if AUR is updated

### Pre-release

```bash
# Create beta release
cargo release --tag-name v1.0.0-beta.1 --execute
```

### Manual Version Override

```bash
# Specify exact version
cargo release 1.2.3 --execute
```

---

## Troubleshooting

### crates.io publish failed
- Check if CRATES_IO_TOKEN is valid
- Ensure all dependencies are published
- Check if version already exists

### Homebrew formula update failed
- Verify HOMEBREW_TAP_DEPLOY_KEY has write access
- Check if homebrew-tap repository exists

### AUR update failed
- Verify SSH key is added to AUR account
- Check PKGBUILD syntax with `makepkg --printsrcinfo`

### winget submission failed
- Manual approval may be required
- Check https://github.com/microsoft/winget-pkgs for PR status

### Chocolatey push failed
- First package requires manual approval
- Check https://community.chocolatey.org/packages/jarvy for status

---

## Security Considerations

1. **Rotate secrets periodically** - Regenerate API keys every 6-12 months
2. **Use minimal scopes** - Only grant necessary permissions
3. **Audit access** - Review who has access to repository secrets
4. **Monitor releases** - Watch for unauthorized release attempts

---

## Useful Commands

```bash
# Check current version
cargo pkgid | cut -d'#' -f2

# Generate changelog without releasing
git-cliff -o CHANGELOG.md

# Dry-run release
cargo release patch --dry-run

# List all tags
git tag -l 'v*'

# View release workflow logs
gh run list --workflow=release.yml
```

---

## Complete Release Checklist

Use this checklist when performing a release:

### Pre-Release
- [ ] All tests pass: `cargo test`
- [ ] Clippy passes: `cargo clippy`
- [ ] CHANGELOG.md is updated
- [ ] Version bumped in Cargo.toml

### Build & Upload
- [ ] GitHub release created with binaries for all platforms:
  - [ ] `jarvy-v{version}-x86_64-apple-darwin.tar.gz` (macOS Intel)
  - [ ] `jarvy-v{version}-aarch64-apple-darwin.tar.gz` (macOS ARM)
  - [ ] `jarvy-v{version}-x86_64-unknown-linux-gnu.tar.gz` (Linux Intel)
  - [ ] `jarvy-v{version}-aarch64-unknown-linux-gnu.tar.gz` (Linux ARM)
  - [ ] `jarvy-v{version}-x86_64-unknown-linux-musl.tar.gz` (Linux musl)
  - [ ] `jarvy-v{version}-x86_64-pc-windows-msvc.zip` (Windows)
  - [ ] `jarvy_{version}_amd64.deb` (Debian package)
  - [ ] `jarvy-{version}-1.x86_64.rpm` (RPM package)

### Package Manager Updates
- [ ] **crates.io**: `cargo publish` (automatic via CI)
- [ ] **Homebrew**: Update `jarvy.rb` with a new version and checksums
- [ ] **AUR**: Update PKGBUILD and push to aur.archlinux.org
- [ ] **winget**: Submit PR to microsoft/winget-pkgs
- [ ] **Chocolatey**: Pack and push to community repository

### Verification
- [ ] Verify crates.io: `cargo install jarvy`
- [ ] Verify Homebrew: `brew upgrade jarvy`
- [ ] Verify AUR: `yay -Syu jarvy-bin`
- [ ] Verify winget: Check PR status
- [ ] Verify Chocolatey: Check moderation status
- [ ] Verify install scripts: Test on fresh machine

---

## Automated vs Manual Steps

| Distribution | Automation Level | Notes |
|--------------|-----------------|-------|
| GitHub Release | Fully Automated | CI builds and uploads binaries |
| crates.io | Fully Automated | CI publishes on tag |
| Homebrew | Semi-Automated | CI creates PR, maintainer approves |
| AUR | Manual | Maintainer updates and pushes |
| Debian (.deb) | Fully Automated | CI builds and attaches to release |
| RPM (.rpm) | Fully Automated | CI builds and attaches to release |
| winget | Manual | Maintainer submits PR, Microsoft approves |
| Chocolatey | Manual | Maintainer packs and pushes |
| Install Scripts | No Updates | Scripts auto-fetch latest release |

---

## Contact

For release issues:
- Open an issue: https://github.com/Cliftonz/jarvy/issues
- Tag with `release` label
