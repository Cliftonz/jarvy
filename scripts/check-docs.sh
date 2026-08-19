#!/usr/bin/env bash
# Local pre-tag docs check — runs the same `mkdocs build --strict` gate
# the release workflow blocks on. Catches broken cross-links, missing
# pages, and other mkdocs-strict drift before the tag is pushed.
#
# Why this exists: the release workflow's `wait_for_docs` gate refuses
# to ship binaries when the docs build fails, but that failure surfaces
# AFTER the tag is already pushed — which means either retagging (noisy)
# or a partially-successful release (rc.4 shipped to crates.io while
# Linux/Windows binaries got stuck behind a doc link fix). Running this
# script locally before `git tag` catches the issue when the fix is a
# one-line edit + amend, not a re-tag.
#
# Usage:
#   ./scripts/check-docs.sh
#
# Requires:
#   - Python 3.12+ with mkdocs-material installed (matches CI):
#       pip install "mkdocs-material[imaging]" cairosvg Pillow
#   - The jarvy binary built at target/release/jarvy (for auto-gen
#     pages via scripts/gen-docs.sh). This script will build it if
#     missing.
#
# Exit codes:
#   0 — docs build clean
#   1 — broken links, missing pages, or any other strict-mode complaint

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Checking mkdocs-material installation..."
if ! command -v mkdocs >/dev/null 2>&1; then
    cat >&2 <<'EOF'
error: mkdocs not on PATH.
Install with:
    pip install "mkdocs-material[imaging]" cairosvg Pillow
EOF
    exit 1
fi

echo "==> Ensuring jarvy binary exists for auto-gen pages..."
if [ ! -x "target/release/jarvy" ]; then
    echo "    target/release/jarvy missing — building..."
    cargo build --bin jarvy --release
fi

echo "==> Regenerating auto-built docs pages..."
# Mirrors the release workflow: gen-docs.sh writes tools-directory,
# cli-reference, etc. from the jarvy binary. RUST_LOG=off silences the
# tracing console layer to keep JSON stdout clean.
JARVY_BIN="$REPO_ROOT/target/release/jarvy" \
    RUST_LOG=off \
    JARVY_TELEMETRY=0 \
    bash scripts/gen-docs.sh

echo "==> Sync Helm chart values.schema.json into docs..."
# Matches docs.yml's step — the published copy at jarvy.dev/schema/...
# must be byte-identical to what Helm enforces.
mkdir -p docs/schema
cp dist/helm/jarvy-telemetry-forwarder/values.schema.json \
    docs/schema/jarvy-telemetry-forwarder.values.schema.json

echo "==> Building docs site (strict mode — same as CI)..."
mkdocs build --strict --site-dir site

echo ""
echo "OK: mkdocs strict build passed. Safe to tag."
