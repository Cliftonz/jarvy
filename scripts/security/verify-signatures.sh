#!/usr/bin/env bash
# verify-signatures.sh - Verify Sigstore signatures for Jarvy releases
# PRD-020: Security Scanning Infrastructure
#
# Usage: ./verify-signatures.sh [VERSION] [ARTIFACT]
#
# Examples:
#   ./verify-signatures.sh v1.0.0                    # Verify all artifacts for v1.0.0
#   ./verify-signatures.sh v1.0.0 jarvy-linux-x86_64.tar.gz  # Verify specific artifact

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Repository information
REPO_OWNER="Cliftonz"
REPO_NAME="jarvy"
GITHUB_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}"
RELEASES_URL="${GITHUB_URL}/releases/download"

# Expected certificate identity and issuer for Sigstore verification
CERT_IDENTITY_REGEXP="https://github.com/${REPO_OWNER}/${REPO_NAME}"
CERT_OIDC_ISSUER="https://token.actions.githubusercontent.com"

# Functions
log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[OK]${NC} $*"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

check_dependencies() {
    local missing=()

    if ! command -v cosign &> /dev/null; then
        missing+=("cosign")
    fi

    if ! command -v curl &> /dev/null; then
        missing+=("curl")
    fi

    if ! command -v sha256sum &> /dev/null && ! command -v shasum &> /dev/null; then
        missing+=("sha256sum or shasum")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing dependencies: ${missing[*]}"
        echo ""
        echo "Installation instructions:"
        echo "  cosign: brew install cosign  OR  https://docs.sigstore.dev/cosign/installation"
        echo "  curl:   Usually pre-installed on most systems"
        exit 1
    fi
}

get_latest_version() {
    curl -sL "https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest" \
        | grep '"tag_name":' \
        | sed -E 's/.*"([^"]+)".*/\1/'
}

download_file() {
    local url="$1"
    local output="$2"

    if curl -sLf -o "$output" "$url"; then
        return 0
    else
        return 1
    fi
}

verify_artifact() {
    local version="$1"
    local artifact="$2"
    local temp_dir
    temp_dir=$(mktemp -d)

    log_info "Verifying: $artifact (version: $version)"

    # Download artifact and signature files
    local artifact_url="${RELEASES_URL}/${version}/${artifact}"
    local sig_url="${RELEASES_URL}/${version}/${artifact}.sig"
    local cert_url="${RELEASES_URL}/${version}/${artifact}.pem"

    log_info "Downloading artifact..."
    if ! download_file "$artifact_url" "${temp_dir}/${artifact}"; then
        log_error "Failed to download artifact: $artifact"
        rm -rf "$temp_dir"
        return 1
    fi

    log_info "Downloading signature..."
    if ! download_file "$sig_url" "${temp_dir}/${artifact}.sig"; then
        log_error "Failed to download signature for: $artifact"
        rm -rf "$temp_dir"
        return 1
    fi

    log_info "Downloading certificate..."
    if ! download_file "$cert_url" "${temp_dir}/${artifact}.pem"; then
        log_error "Failed to download certificate for: $artifact"
        rm -rf "$temp_dir"
        return 1
    fi

    # Verify with cosign
    log_info "Verifying Sigstore signature..."
    if cosign verify-blob \
        --signature "${temp_dir}/${artifact}.sig" \
        --certificate "${temp_dir}/${artifact}.pem" \
        --certificate-identity-regexp "$CERT_IDENTITY_REGEXP" \
        --certificate-oidc-issuer "$CERT_OIDC_ISSUER" \
        "${temp_dir}/${artifact}" 2>&1; then
        log_success "Signature verified successfully for: $artifact"
    else
        log_error "Signature verification FAILED for: $artifact"
        rm -rf "$temp_dir"
        return 1
    fi

    # Clean up
    rm -rf "$temp_dir"
    return 0
}

verify_checksum() {
    local version="$1"
    local artifact="$2"
    local temp_dir
    temp_dir=$(mktemp -d)

    log_info "Verifying checksum for: $artifact"

    # Download artifact and checksums
    local artifact_url="${RELEASES_URL}/${version}/${artifact}"
    local checksums_url="${RELEASES_URL}/${version}/SHA256SUMS.txt"

    if ! download_file "$artifact_url" "${temp_dir}/${artifact}"; then
        log_error "Failed to download artifact: $artifact"
        rm -rf "$temp_dir"
        return 1
    fi

    if ! download_file "$checksums_url" "${temp_dir}/SHA256SUMS.txt"; then
        log_error "Failed to download checksums file"
        rm -rf "$temp_dir"
        return 1
    fi

    # Calculate and verify checksum
    local expected_checksum
    expected_checksum=$(grep "$artifact" "${temp_dir}/SHA256SUMS.txt" | awk '{print $1}')

    if [ -z "$expected_checksum" ]; then
        log_warning "Artifact not found in checksums file: $artifact"
        rm -rf "$temp_dir"
        return 1
    fi

    local actual_checksum
    if command -v sha256sum &> /dev/null; then
        actual_checksum=$(sha256sum "${temp_dir}/${artifact}" | awk '{print $1}')
    else
        actual_checksum=$(shasum -a 256 "${temp_dir}/${artifact}" | awk '{print $1}')
    fi

    if [ "$expected_checksum" = "$actual_checksum" ]; then
        log_success "Checksum verified: $artifact"
    else
        log_error "Checksum mismatch for: $artifact"
        log_error "  Expected: $expected_checksum"
        log_error "  Actual:   $actual_checksum"
        rm -rf "$temp_dir"
        return 1
    fi

    rm -rf "$temp_dir"
    return 0
}

list_artifacts() {
    local version="$1"

    curl -sL "https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/tags/${version}" \
        | grep '"name":' \
        | grep -E '\.(tar\.gz|zip|deb|rpm|dmg|msi|exe|AppImage)$' \
        | sed -E 's/.*"([^"]+)".*/\1/'
}

usage() {
    echo "Usage: $0 [OPTIONS] [VERSION] [ARTIFACT]"
    echo ""
    echo "Verify Sigstore signatures for Jarvy releases"
    echo ""
    echo "Arguments:"
    echo "  VERSION     Release version (e.g., v1.0.0). Default: latest"
    echo "  ARTIFACT    Specific artifact to verify. Default: all artifacts"
    echo ""
    echo "Options:"
    echo "  -h, --help          Show this help message"
    echo "  -l, --list          List available artifacts for the version"
    echo "  -c, --checksum-only Verify only SHA256 checksums (no Sigstore)"
    echo ""
    echo "Examples:"
    echo "  $0                           # Verify all artifacts for latest release"
    echo "  $0 v1.0.0                    # Verify all artifacts for v1.0.0"
    echo "  $0 v1.0.0 jarvy-linux.tar.gz # Verify specific artifact"
    echo "  $0 -l v1.0.0                 # List artifacts for v1.0.0"
}

main() {
    local version=""
    local artifact=""
    local list_only=false
    local checksum_only=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                usage
                exit 0
                ;;
            -l|--list)
                list_only=true
                shift
                ;;
            -c|--checksum-only)
                checksum_only=true
                shift
                ;;
            -*)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
            *)
                if [ -z "$version" ]; then
                    version="$1"
                elif [ -z "$artifact" ]; then
                    artifact="$1"
                else
                    log_error "Too many arguments"
                    usage
                    exit 1
                fi
                shift
                ;;
        esac
    done

    # Check dependencies
    check_dependencies

    # Get version
    if [ -z "$version" ]; then
        log_info "Fetching latest version..."
        version=$(get_latest_version)
        if [ -z "$version" ]; then
            log_error "Could not determine latest version"
            exit 1
        fi
        log_info "Latest version: $version"
    fi

    # List only mode
    if $list_only; then
        log_info "Artifacts for $version:"
        list_artifacts "$version"
        exit 0
    fi

    # Verify specific artifact or all
    if [ -n "$artifact" ]; then
        if $checksum_only; then
            verify_checksum "$version" "$artifact"
        else
            verify_artifact "$version" "$artifact"
        fi
    else
        log_info "Verifying all artifacts for $version..."
        local artifacts
        artifacts=$(list_artifacts "$version")

        if [ -z "$artifacts" ]; then
            log_error "No artifacts found for version: $version"
            exit 1
        fi

        local failed=0
        for art in $artifacts; do
            echo ""
            if $checksum_only; then
                verify_checksum "$version" "$art" || ((failed++))
            else
                verify_artifact "$version" "$art" || ((failed++))
            fi
        done

        echo ""
        if [ $failed -eq 0 ]; then
            log_success "All artifacts verified successfully!"
        else
            log_error "$failed artifact(s) failed verification"
            exit 1
        fi
    fi
}

main "$@"
