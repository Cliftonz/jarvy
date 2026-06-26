#!/usr/bin/env bash
# generate-sbom.sh - Generate Software Bill of Materials for Jarvy
# PRD-020: Security Scanning Infrastructure
#
# Usage: ./generate-sbom.sh [OPTIONS]
#
# Generates SBOM in SPDX and/or CycloneDX formats

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
OUTPUT_DIR="."
FORMAT="both"  # spdx, cyclonedx, both
VERBOSE=false

# Functions
log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[OK]${NC} $*"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }
log_verbose() { $VERBOSE && echo -e "${BLUE}[DEBUG]${NC} $*" || true; }

check_dependencies() {
    local missing=()

    if ! command -v cargo &> /dev/null; then
        log_error "cargo is required but not installed"
        exit 1
    fi

    if ! command -v jq &> /dev/null; then
        missing+=("jq")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        log_warning "Optional dependencies missing: ${missing[*]}"
        log_warning "Some features may not work without jq"
    fi
}

get_version() {
    cargo metadata --format-version 1 --no-deps 2>/dev/null \
        | jq -r '.packages[] | select(.name == "jarvy") | .version' 2>/dev/null \
        || grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'
}

install_sbom_tools() {
    log_info "Checking SBOM tools..."

    if ! command -v cargo-sbom &> /dev/null; then
        log_info "Installing cargo-sbom..."
        cargo install cargo-sbom --locked 2>/dev/null || {
            log_warning "Failed to install cargo-sbom, will use fallback"
        }
    fi

    if ! command -v cargo-cyclonedx &> /dev/null; then
        log_info "Installing cargo-cyclonedx..."
        cargo install cargo-cyclonedx --locked 2>/dev/null || {
            log_warning "Failed to install cargo-cyclonedx, will use fallback"
        }
    fi
}

generate_spdx() {
    local output_file="${OUTPUT_DIR}/sbom.spdx.json"
    local version
    version=$(get_version)

    log_info "Generating SPDX SBOM..."

    # Try cargo-sbom first
    if command -v cargo-sbom &> /dev/null; then
        log_verbose "Using cargo-sbom"
        if cargo sbom --output-format spdx_json_2_3 > "$output_file" 2>/dev/null; then
            log_success "Generated SPDX SBOM: $output_file"
            return 0
        fi
    fi

    # Fallback: Generate from cargo metadata
    log_verbose "Using cargo metadata fallback"
    if command -v jq &> /dev/null; then
        cargo metadata --format-version 1 | jq --arg version "$version" '{
            spdxVersion: "SPDX-2.3",
            dataLicense: "CC0-1.0",
            SPDXID: "SPDXRef-DOCUMENT",
            name: "jarvy-sbom",
            documentNamespace: ("https://github.com/Cliftonz/jarvy/releases/tag/v" + $version),
            creationInfo: {
                created: (now | strftime("%Y-%m-%dT%H:%M:%SZ")),
                creators: ["Tool: cargo-metadata", "Organization: Jarvy"]
            },
            packages: [.packages[] | {
                SPDXID: ("SPDXRef-Package-" + .name + "-" + .version),
                name: .name,
                versionInfo: .version,
                downloadLocation: (.repository // "NOASSERTION"),
                filesAnalyzed: false,
                licenseConcluded: (.license // "NOASSERTION"),
                licenseDeclared: (.license // "NOASSERTION"),
                copyrightText: "NOASSERTION",
                externalRefs: [{
                    referenceCategory: "PACKAGE-MANAGER",
                    referenceType: "purl",
                    referenceLocator: ("pkg:cargo/" + .name + "@" + .version)
                }]
            }],
            relationships: [.packages[] | {
                spdxElementId: "SPDXRef-DOCUMENT",
                relatedSpdxElement: ("SPDXRef-Package-" + .name + "-" + .version),
                relationshipType: "DESCRIBES"
            }]
        }' > "$output_file"
        log_success "Generated SPDX SBOM (fallback): $output_file"
    else
        log_error "Cannot generate SPDX SBOM: jq is required for fallback"
        return 1
    fi
}

generate_cyclonedx() {
    local output_file="${OUTPUT_DIR}/sbom.cdx.json"
    local version
    version=$(get_version)

    log_info "Generating CycloneDX SBOM..."

    # Try cargo-cyclonedx first
    if command -v cargo-cyclonedx &> /dev/null; then
        log_verbose "Using cargo-cyclonedx"
        if cargo cyclonedx --format json > "$output_file" 2>/dev/null; then
            log_success "Generated CycloneDX SBOM: $output_file"
            return 0
        fi
    fi

    # Fallback: Generate from cargo metadata
    log_verbose "Using cargo metadata fallback"
    if command -v jq &> /dev/null; then
        cargo metadata --format-version 1 | jq --arg version "$version" '{
            bomFormat: "CycloneDX",
            specVersion: "1.4",
            serialNumber: ("urn:uuid:" + (now | tostring | @base64 | .[0:36])),
            version: 1,
            metadata: {
                timestamp: (now | strftime("%Y-%m-%dT%H:%M:%SZ")),
                tools: [{
                    vendor: "Jarvy",
                    name: "generate-sbom.sh",
                    version: "1.0.0"
                }],
                component: {
                    type: "application",
                    name: "jarvy",
                    version: $version,
                    purl: ("pkg:cargo/jarvy@" + $version)
                }
            },
            components: [.packages[] | select(.name != "jarvy") | {
                type: "library",
                name: .name,
                version: .version,
                purl: ("pkg:cargo/" + .name + "@" + .version),
                licenses: (if .license then [{license: {id: .license}}] else [] end),
                externalReferences: (if .repository then [{type: "vcs", url: .repository}] else [] end)
            }]
        }' > "$output_file"
        log_success "Generated CycloneDX SBOM (fallback): $output_file"
    else
        log_error "Cannot generate CycloneDX SBOM: jq is required for fallback"
        return 1
    fi
}

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Generate Software Bill of Materials (SBOM) for Jarvy"
    echo ""
    echo "Options:"
    echo "  -o, --output DIR     Output directory (default: current directory)"
    echo "  -f, --format FORMAT  Output format: spdx, cyclonedx, both (default: both)"
    echo "  -i, --install        Install SBOM tools if missing"
    echo "  -v, --verbose        Enable verbose output"
    echo "  -h, --help           Show this help message"
    echo ""
    echo "Output files:"
    echo "  sbom.spdx.json      SPDX 2.3 format"
    echo "  sbom.cdx.json       CycloneDX 1.4 format"
    echo ""
    echo "Examples:"
    echo "  $0                          # Generate both formats in current directory"
    echo "  $0 -f spdx -o dist/         # Generate SPDX only in dist/"
    echo "  $0 -i -v                    # Install tools and generate with verbose output"
}

main() {
    local install_tools=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                usage
                exit 0
                ;;
            -o|--output)
                OUTPUT_DIR="$2"
                shift 2
                ;;
            -f|--format)
                FORMAT="$2"
                shift 2
                ;;
            -i|--install)
                install_tools=true
                shift
                ;;
            -v|--verbose)
                VERBOSE=true
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done

    # Validate format
    case "$FORMAT" in
        spdx|cyclonedx|both) ;;
        *)
            log_error "Invalid format: $FORMAT"
            log_error "Valid formats: spdx, cyclonedx, both"
            exit 1
            ;;
    esac

    # Check we're in a Rust project
    if [ ! -f "Cargo.toml" ]; then
        log_error "Cargo.toml not found. Please run from project root."
        exit 1
    fi

    # Create output directory
    mkdir -p "$OUTPUT_DIR"

    # Check dependencies
    check_dependencies

    # Install tools if requested
    if $install_tools; then
        install_sbom_tools
    fi

    # Generate SBOMs
    local failed=0

    case "$FORMAT" in
        spdx)
            generate_spdx || ((failed++))
            ;;
        cyclonedx)
            generate_cyclonedx || ((failed++))
            ;;
        both)
            generate_spdx || ((failed++))
            generate_cyclonedx || ((failed++))
            ;;
    esac

    echo ""
    if [ $failed -eq 0 ]; then
        log_success "SBOM generation complete!"
        log_info "Output directory: $OUTPUT_DIR"
    else
        log_error "SBOM generation completed with errors"
        exit 1
    fi
}

main "$@"
