#!/bin/bash
# Jarvy Installer Script
# Usage: curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash
#
# Environment variables:
#   JARVY_VERSION        - Version to install (default: latest)
#   JARVY_CHANNEL        - Release channel: stable (default), beta, nightly
#                          beta accepts -rc.N and -beta.N tags
#                          nightly accepts every tag including -alpha.N
#   JARVY_INSTALL_DIR    - Installation directory (default: ~/.local/bin)
#   JARVY_NO_MODIFY_PATH - Set to 1 to skip PATH modification
#   JARVY_SKIP_CHECKSUM  - Set to 1 to skip SHA256 integrity verification
#                          (NOT recommended — bypasses supply-chain check)

set -euo pipefail

JARVY_VERSION="${JARVY_VERSION:-latest}"
JARVY_CHANNEL="${JARVY_CHANNEL:-stable}"
JARVY_INSTALL_DIR="${JARVY_INSTALL_DIR:-$HOME/.local/bin}"
JARVY_REPO="Cliftonz/jarvy"
JARVY_NO_MODIFY_PATH="${JARVY_NO_MODIFY_PATH:-0}"
JARVY_SKIP_CHECKSUM="${JARVY_SKIP_CHECKSUM:-0}"

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

# Map a normalized Linux architecture to the release triple that
# release.yml actually publishes. The x86_64 build is static musl (runs
# on glibc AND musl); aarch64 ships as gnu; armv7 as gnueabihf. The
# detected libc is deliberately ignored: requesting an unpublished
# combination — e.g. x86_64-gnu on a glibc box, the common case — would
# 404 because that asset was never built. Prints the triple, or returns
# 1 for an unsupported arch.
resolve_linux_triple() {
    case "$1" in
        x86_64|amd64)  echo "x86_64-unknown-linux-musl" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        armv7|armv7l)  echo "armv7-unknown-linux-gnueabihf" ;;
        *)             return 1 ;;
    esac
}

# Detect OS and architecture
detect_platform() {
    local os arch triple

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
        log_error "    brew install Cliftonz/tap/jarvy"
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

    # Linux: request the triple release.yml actually shipped, not the
    # ${arch}-${libc} guess (which 404s for glibc x86_64 / musl arm64).
    case "$os" in
        unknown-linux-*)
            if triple="$(resolve_linux_triple "$arch")"; then
                echo "$triple"
                return 0
            fi
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
# Fetch a GitHub API URL with auth (when GITHUB_TOKEN / GH_TOKEN is set,
# for the 5000/hr authenticated rate limit instead of 60/hr) and up to 3
# retries with linear backoff — a single transient 403/rate-limit or
# network blip should not hard-fail an install. Echoes the body on
# success; returns 1 after exhausting retries.
gh_api_fetch() {
    local url="$1"
    local token="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
    local -a auth=()
    [ -n "$token" ] && auth=(-H "Authorization: Bearer $token")
    local attempt body
    for attempt in 1 2 3; do
        # `${auth[@]+"${auth[@]}"}` expands to nothing when the array is
        # empty and to the quoted elements otherwise. macOS ships bash
        # 3.2, where a bare `"${auth[@]}"` on an empty array under
        # `set -u` is an "unbound variable" error (bash 4.4+ treats it as
        # empty) — this idiom is safe on both.
        if body=$(curl -fsSL ${auth[@]+"${auth[@]}"} -H "X-GitHub-Api-Version: 2022-11-28" "$url" 2>/dev/null) \
            && [ -n "$body" ]; then
            echo "$body"
            return 0
        fi
        [ "$attempt" -lt 3 ] && sleep "$attempt"
    done
    return 1
}

get_latest_version() {
    if [ "$JARVY_CHANNEL" = "stable" ]; then
        local response
        if ! response=$(gh_api_fetch "https://api.github.com/repos/${JARVY_REPO}/releases/latest"); then
            log_error "Failed to fetch latest stable release from GitHub (rate limit? set GITHUB_TOKEN)"
            exit 1
        fi
        echo "$response" | grep '"tag_name"' | head -1 | sed -E 's/.*"v?([^"]+)".*/\1/'
        return 0
    fi

    # beta or nightly
    local response
    if ! response=$(gh_api_fetch "https://api.github.com/repos/${JARVY_REPO}/releases?per_page=30"); then
        log_error "Failed to fetch releases from GitHub (rate limit? set GITHUB_TOKEN)"
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

# Fetch the expected SHA256 for a given archive from the release's
# SHA256SUMS.txt. Prints the hex digest on stdout and returns 0 on
# success; returns 1 if the sums file is unreachable or the archive
# is not listed. SHA256SUMS.txt lines are "<hex>  [./]<filename>"
# (the `./` prefix comes from release.yml's `sha256sum ./jarvy*`).
fetch_expected_sha() {
    local version="$1"
    local archive_name="$2"
    local sums_url sums
    sums_url="https://github.com/${JARVY_REPO}/releases/download/v${version}/SHA256SUMS.txt"

    # Retry: the SHA256SUMS.txt asset exists for every release, so a
    # failure here is almost always a transient release-CDN blip or
    # rate-limit rather than a genuinely missing file. Retry before
    # falling back to the (warned) checksum skip.
    local attempt
    for attempt in 1 2 3; do
        sums="$(curl -fsSL "$sums_url" 2>/dev/null)" && [ -n "$sums" ] && break
        sums=""
        [ "$attempt" -lt 3 ] && sleep "$attempt"
    done
    [ -n "$sums" ] || return 1

    # Match by BASENAME: entries carry build paths (./release/jarvy-*.tar.gz,
    # ./generate-rpm/jarvy-*.rpm), so stripping only a leading `./` never
    # matched pathed entries and verification silently fell through to the
    # warn-and-proceed branch — on every release with pathed entries (caught
    # by installer-e2e's first-ever run, 2026-07-15). awk exits 1 when no
    # line matched so the caller can distinguish "missing entry" from
    # "empty digest".
    echo "$sums" | awk -v want="$archive_name" '
        {
            name = $2
            gsub(/^.*\//, "", name)
            if (name == want) { print $1; found = 1; exit }
        }
        END { if (!found) exit 1 }
    '
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

    # Create temporary directory. Bake the resolved path into the trap
    # command NOW (double quotes expand at registration) rather than
    # referencing $tmp_dir lazily: `tmp_dir` is a `local` in main(), but
    # the EXIT trap fires at *script* exit when that local is out of
    # scope — under `set -u` a lazy `$tmp_dir` then throws "unbound
    # variable" and the trap exits 1, failing every install after a
    # successful download. mktemp -d output contains no quotes, so the
    # single-quoted embedding is safe.
    tmp_dir=$(mktemp -d)
    # shellcheck disable=SC2064  # expand-now is the intent (see comment above)
    trap "rm -rf '$tmp_dir'" EXIT

    # Download
    log_info "Downloading..."
    if ! curl -fsSL "$url" -o "$tmp_dir/jarvy.$archive_ext"; then
        log_error "Download failed. Please check if the version exists."
        exit 1
    fi

    # Verify integrity against the release SHA256SUMS.txt before we ever
    # extract or execute the downloaded bytes. A mismatch aborts — a
    # missing sums file (very old releases) warns but proceeds so a
    # legacy tag stays installable. JARVY_SKIP_CHECKSUM=1 opts out.
    local archive_name expected_sha
    archive_name="jarvy-v${version}-${platform}.${archive_ext}"
    if [ "$JARVY_SKIP_CHECKSUM" = "1" ]; then
        log_warn "JARVY_SKIP_CHECKSUM=1 set — skipping integrity verification"
    elif expected_sha="$(fetch_expected_sha "$version" "$archive_name")" && [ -n "$expected_sha" ]; then
        if ! verify_checksum "$tmp_dir/jarvy.$archive_ext" "$expected_sha"; then
            log_error "Refusing to install: downloaded archive failed checksum verification."
            exit 1
        fi
    else
        log_warn "SHA256SUMS.txt not found for v${version} — skipping integrity check."
        log_warn "Set JARVY_SKIP_CHECKSUM=1 to silence, or verify the download manually."
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
    echo "    jarvy --help                # Show help"
    echo "    jarvy configure             # Create jarvy.toml"
    echo "    jarvy setup                 # Install tools"
    echo "    jarvy shell-init --apply    # Set up 'jr' (jarvy run) in your shell"
    echo ""
}

# Only auto-run when executed directly, not when sourced (the unit tests
# in dist/scripts/tests/ source this file to exercise individual
# functions without triggering a real install).
if [ "${BASH_SOURCE[0]:-$0}" = "${0}" ]; then
    main "$@"
fi
