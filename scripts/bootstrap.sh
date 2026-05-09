#!/usr/bin/env bash
# bootstrap.sh — clean-laptop onboarding for Jarvy contributors.
#
# Installs Jarvy if missing, then runs `jarvy setup` against jarvy.toml in the
# repo root. Idempotent: safe to re-run after a vacation, after pulling main,
# or whenever the environment drifts.
#
# Usage:
#   ./scripts/bootstrap.sh                 # install + setup
#   ./scripts/bootstrap.sh --no-setup      # only ensure jarvy is installed
#   ./scripts/bootstrap.sh --channel beta  # use jarvy beta channel for install

set -euo pipefail

CHANNEL="${JARVY_CHANNEL:-stable}"
RUN_SETUP=1
EXTRA_SETUP_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-setup)
            RUN_SETUP=0
            shift
            ;;
        --channel)
            CHANNEL="$2"
            shift 2
            ;;
        --channel=*)
            CHANNEL="${1#*=}"
            shift
            ;;
        --)
            shift
            EXTRA_SETUP_ARGS+=("$@")
            break
            ;;
        *)
            EXTRA_SETUP_ARGS+=("$1")
            shift
            ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

color() { printf '\033[%sm%s\033[0m' "$1" "$2"; }
info()  { printf '%s %s\n' "$(color '1;34' '==>')" "$*"; }
warn()  { printf '%s %s\n' "$(color '1;33' '==>')" "$*" >&2; }
err()   { printf '%s %s\n' "$(color '1;31' '==>')" "$*" >&2; }

have() { command -v "$1" >/dev/null 2>&1; }

install_jarvy() {
    info "Installing Jarvy (channel: $CHANNEL)..."

    if have curl; then
        JARVY_CHANNEL="$CHANNEL" \
            curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
    elif have wget; then
        JARVY_CHANNEL="$CHANNEL" \
            wget -qO- https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
    else
        err "Neither curl nor wget found. Install one and re-run."
        exit 1
    fi

    # install.sh drops the binary into ~/.cargo/bin or /usr/local/bin —
    # make sure the current shell can find it.
    for candidate in "$HOME/.cargo/bin" "$HOME/.local/bin" "/usr/local/bin" "/opt/homebrew/bin"; do
        if [[ -x "$candidate/jarvy" && ":$PATH:" != *":$candidate:"* ]]; then
            export PATH="$candidate:$PATH"
        fi
    done
}

if have jarvy; then
    info "Jarvy already installed: $(jarvy --version)"
else
    install_jarvy
    if ! have jarvy; then
        err "Jarvy installed but not on PATH. Open a new shell, or add the install dir to PATH, then re-run."
        exit 1
    fi
    info "Installed: $(jarvy --version)"
fi

if [[ "$RUN_SETUP" -eq 0 ]]; then
    info "Skipping setup (--no-setup)."
    exit 0
fi

if [[ ! -f "$REPO_ROOT/jarvy.toml" ]]; then
    warn "No jarvy.toml at $REPO_ROOT — nothing to provision."
    exit 0
fi

info "Running jarvy setup..."
exec jarvy setup "${EXTRA_SETUP_ARGS[@]}"
