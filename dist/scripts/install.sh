#!/bin/bash
# Jarvy Installer Script
# Usage: curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
#
# Environment variables:
#   JARVY_VERSION        - Version to install (default: latest)
#   JARVY_CHANNEL        - Release channel: stable (default), beta, nightly
#                          beta accepts -rc.N and -beta.N tags
#                          nightly accepts every tag including -alpha.N
#   JARVY_INSTALL_DIR    - Installation directory (default: ~/.local/bin)
#   JARVY_NO_MODIFY_PATH - Set to 1 to skip PATH modification

set -euo pipefail

JARVY_VERSION="${JARVY_VERSION:-latest}"
JARVY_CHANNEL="${JARVY_CHANNEL:-stable}"
JARVY_INSTALL_DIR="${JARVY_INSTALL_DIR:-$HOME/.local/bin}"
JARVY_REPO="bearbinary/jarvy"
JARVY_NO_MODIFY_PATH="${JARVY_NO_MODIFY_PATH:-0}"

# Validate channel
case "$JARVY_CHANNEL" in
    stable|beta|nightly) ;;
    *)
        echo "ERROR: Unknown JARVY_CHANNEL '$JARVY_CHANNEL'. Expected: stable, beta, nightly." >&2
        exit 1
        ;;
esac

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# Detect OS and architecture
detect_platform() {
    local os arch

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin)
            os="apple-darwin"
            ;;
        Linux)
            # Check if musl-based (Alpine, etc.)
            if ldd --version 2>&1 | grep -q musl; then
                os="unknown-linux-musl"
            else
                os="unknown-linux-gnu"
            fi
            ;;
        MINGW*|MSYS*|CYGWIN*)
            os="pc-windows-msvc"
            ;;
        *)
            log_error "Unsupported OS: $os"
            exit 1
            ;;
    esac

    # macOS Intel (x86_64) is not shipped as a prebuilt binary as of
    # v0.1.0. Auto-fall-through to `cargo install jarvy` when cargo is
    # present; if cargo is missing, surface a clear bootstrap step
    # rather than a download 404.
    if [ "$os" = "apple-darwin" ] && { [ "$arch" = "x86_64" ] || [ "$arch" = "amd64" ]; }; then
        log_info "Intel macOS detected: prebuilt .dmg not shipped for this arch."
        if command -v cargo >/dev/null 2>&1; then
            log_info "Installing via cargo install jarvy (compiles from source, ~2 min)..."
            if [ "$JARVY_VERSION" = "latest" ]; then
                cargo install jarvy
            else
                cargo install jarvy --version "${JARVY_VERSION#v}"
            fi
            log_success "Jarvy installed via cargo. Run 'jarvy --version' to verify."
            exit 0
        fi
        log_error "cargo not found and Intel macOS is not in the prebuilt matrix."
        log_error ""
        log_error "Install Rust first, then re-run this script:"
        log_error "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        log_error ""
        log_error "Or install Jarvy via Homebrew (compiles from source on Intel):"
        log_error "    brew install bearbinary/tap/jarvy"
        exit 1
    fi

    case "$arch" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        aarch64|arm64)
            arch="aarch64"
            ;;
        armv7l)
            arch="armv7"
            ;;
        *)
            log_error "Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    echo "${arch}-${os}"
}

# Channel filter: returns 0 if the given tag matches JARVY_CHANNEL.
# stable rejects any tag containing '-'.
# beta accepts plain tags plus -rc.N / -beta.N.
# nightly accepts all tags.
matches_channel() {
    local tag="$1"
    case "$JARVY_CHANNEL" in
        stable)
            [[ "$tag" != *-* ]]
            ;;
        beta)
            [[ "$tag" != *-* ]] || [[ "$tag" == *-rc.* ]] || [[ "$tag" == *-beta.* ]]
            ;;
        nightly)
            return 0
            ;;
    esac
}

# Get latest version from GitHub API for the configured channel.
# stable -> /releases/latest (skips drafts and prereleases).
# beta/nightly -> /releases (includes prereleases); pick the first tag that
# matches the channel filter.
get_latest_version() {
    if [ "$JARVY_CHANNEL" = "stable" ]; then
        local response
        response=$(curl -fsSL "https://api.github.com/repos/${JARVY_REPO}/releases/latest" 2>/dev/null)
        if [ -z "$response" ]; then
            log_error "Failed to fetch latest stable release from GitHub"
            exit 1
        fi
        echo "$response" | grep '"tag_name"' | head -1 | sed -E 's/.*"v?([^"]+)".*/\1/'
        return 0
    fi

    # beta or nightly
    local response
    response=$(curl -fsSL "https://api.github.com/repos/${JARVY_REPO}/releases?per_page=30" 2>/dev/null)
    if [ -z "$response" ]; then
        log_error "Failed to fetch releases from GitHub"
        exit 1
    fi

    # GitHub API returns releases in chronological order (newest first).
    # Walk tag_name lines until one matches the channel.
    local tag
    while IFS= read -r tag; do
        # tag arrives as "vX.Y.Z" or "vX.Y.Z-rc.N"
        if matches_channel "$tag"; then
            echo "${tag#v}"
            return 0
        fi
    done < <(echo "$response" | grep '"tag_name"' | sed -E 's/.*"(v?[^"]+)".*/\1/')

    log_error "No release matching channel '$JARVY_CHANNEL' found in the most recent 30 releases"
    exit 1
}

# Verify checksum
verify_checksum() {
    local file="$1"
    local expected_sha="$2"

    if command -v sha256sum &>/dev/null; then
        actual_sha=$(sha256sum "$file" | awk '{print $1}')
    elif command -v shasum &>/dev/null; then
        actual_sha=$(shasum -a 256 "$file" | awk '{print $1}')
    else
        log_warn "sha256sum/shasum not found, skipping checksum verification"
        return 0
    fi

    if [ "$actual_sha" != "$expected_sha" ]; then
        log_error "Checksum verification failed!"
        log_error "Expected: $expected_sha"
        log_error "Actual:   $actual_sha"
        return 1
    fi

    log_info "Checksum verified"
    return 0
}

# Add to PATH
add_to_path() {
    local install_dir="$1"
    local shell_rc=""
    local path_line="export PATH=\"$install_dir:\$PATH\""

    # Detect shell config file
    if [ -n "${SHELL:-}" ]; then
        case "$SHELL" in
            */zsh)
                shell_rc="$HOME/.zshrc"
                ;;
            */bash)
                if [ -f "$HOME/.bashrc" ]; then
                    shell_rc="$HOME/.bashrc"
                elif [ -f "$HOME/.bash_profile" ]; then
                    shell_rc="$HOME/.bash_profile"
                fi
                ;;
            */fish)
                shell_rc="$HOME/.config/fish/config.fish"
                path_line="set -gx PATH $install_dir \$PATH"
                ;;
        esac
    fi

    # Fallback to common files
    if [ -z "$shell_rc" ]; then
        if [ -f "$HOME/.zshrc" ]; then
            shell_rc="$HOME/.zshrc"
        elif [ -f "$HOME/.bashrc" ]; then
            shell_rc="$HOME/.bashrc"
        fi
    fi

    if [ -n "$shell_rc" ] && [ -f "$shell_rc" ]; then
        if ! grep -q "$install_dir" "$shell_rc" 2>/dev/null; then
            echo "" >> "$shell_rc"
            echo "# Added by Jarvy installer" >> "$shell_rc"
            echo "$path_line" >> "$shell_rc"
            log_info "Added $install_dir to PATH in $shell_rc"
        fi
    fi
}

main() {
    local platform version url archive_ext tmp_dir

    log_info "Jarvy Installer"
    echo ""

    # Detect platform
    platform="$(detect_platform)"
    log_info "Detected platform: $platform"

    # Get version
    if [ "$JARVY_VERSION" = "latest" ]; then
        log_info "Channel: $JARVY_CHANNEL"
        log_info "Fetching latest version on '$JARVY_CHANNEL' channel..."
        version="$(get_latest_version)"
    else
        version="${JARVY_VERSION#v}"  # Remove 'v' prefix if present
    fi
    log_info "Installing version: v$version"

    # Determine archive extension
    if [[ "$platform" == *"windows"* ]]; then
        archive_ext="zip"
    else
        archive_ext="tar.gz"
    fi

    # Build download URL
    url="https://github.com/${JARVY_REPO}/releases/download/v${version}/jarvy-v${version}-${platform}.${archive_ext}"
    log_info "Download URL: $url"

    # Create temporary directory
    tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' EXIT

    # Download
    log_info "Downloading..."
    if ! curl -fsSL "$url" -o "$tmp_dir/jarvy.$archive_ext"; then
        log_error "Download failed. Please check if the version exists."
        exit 1
    fi

    # Extract
    log_info "Extracting..."
    if [ "$archive_ext" = "zip" ]; then
        unzip -q "$tmp_dir/jarvy.zip" -d "$tmp_dir"
    else
        tar -xzf "$tmp_dir/jarvy.tar.gz" -C "$tmp_dir"
    fi

    # Install
    log_info "Installing to $JARVY_INSTALL_DIR..."
    mkdir -p "$JARVY_INSTALL_DIR"
    cp "$tmp_dir/jarvy" "$JARVY_INSTALL_DIR/jarvy" 2>/dev/null || \
        cp "$tmp_dir/jarvy.exe" "$JARVY_INSTALL_DIR/jarvy.exe" 2>/dev/null || \
        cp "$tmp_dir/"*/jarvy* "$JARVY_INSTALL_DIR/" 2>/dev/null
    chmod +x "$JARVY_INSTALL_DIR/jarvy" 2>/dev/null || true

    log_success "Jarvy v$version installed to $JARVY_INSTALL_DIR/jarvy"

    # Check if in PATH
    if ! command -v jarvy &>/dev/null; then
        if [ "$JARVY_NO_MODIFY_PATH" != "1" ]; then
            add_to_path "$JARVY_INSTALL_DIR"
            echo ""
            log_warn "PATH updated. Please restart your terminal or run:"
            echo "    source ~/.bashrc  # or ~/.zshrc"
        else
            echo ""
            log_info "Add the following to your PATH:"
            echo "    export PATH=\"$JARVY_INSTALL_DIR:\$PATH\""
        fi
    fi

    echo ""
    log_success "Installation complete!"
    echo ""
    echo "Get started:"
    echo "    jarvy --help      # Show help"
    echo "    jarvy configure   # Create jarvy.toml"
    echo "    jarvy setup       # Install tools"
    echo ""
}

main "$@"
