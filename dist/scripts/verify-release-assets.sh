#!/usr/bin/env bash
#
# verify-release-assets.sh — defense-in-depth sweep over a Jarvy GitHub release.
#
# For the given tag, fetches the release manifest and checks every asset:
#   1. HTTP 200 on its browser_download_url
#   2. Listed in SHA256SUMS.txt with a checksum that matches the downloaded file
#   3. Has a matching .sig / .pem / .bundle triple
#   4. cosign verify-blob succeeds against the Sigstore bundle
#   5. SBOM artifacts present and parseable as CycloneDX or SPDX JSON
#   6. For binaries matching the current OS+arch, `<bin> --version` reports the tag
#
# Exits 0 on a clean sweep. Exits non-zero with a categorised failure on the
# first hard fail. This script is the gate behind Path 8 of docs/release-testing.md.
#
# Requires: gh, jq, curl, cosign, sha256sum (or shasum -a 256), tar, unzip.

set -euo pipefail

TAG="${1:-}"
if [[ -z "$TAG" ]]; then
  echo "usage: $0 <tag>   e.g. $0 v0.2.0-rc.1" >&2
  exit 64
fi

REPO="${JARVY_REPO:-bearbinary/Jarvy}"
WORKDIR="$(mktemp -d -t jarvy-verify-XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

VERSION="${TAG#v}"
ASSETS_DIR="$WORKDIR/assets"
mkdir -p "$ASSETS_DIR"

step() { printf '\n\033[1;34m==>\033[0m %s\n' "$*"; }
ok()   { printf '  \033[1;32m✓\033[0m %s\n' "$*"; }
fail() { printf '  \033[1;31m✗\033[0m %s\n' "$*" >&2; exit 1; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

step "Preflight"
for c in gh jq curl cosign tar; do require_cmd "$c"; done
command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 \
  || fail "need sha256sum or shasum"
ok "tools present"

step "Fetching release manifest for $TAG"
MANIFEST="$WORKDIR/release.json"
gh release view "$TAG" --repo "$REPO" --json tagName,assets,isPrerelease,isDraft \
  >"$MANIFEST" || fail "release $TAG not found in $REPO"

IS_DRAFT="$(jq -r '.isDraft' "$MANIFEST")"
[[ "$IS_DRAFT" == "false" ]] || fail "release is still draft — publish before verifying"
ok "release found, isPrerelease=$(jq -r .isPrerelease "$MANIFEST")"

ASSET_COUNT="$(jq '.assets | length' "$MANIFEST")"
[[ "$ASSET_COUNT" -gt 0 ]] || fail "release has zero assets"
ok "$ASSET_COUNT assets advertised"

step "Downloading every asset and asserting HTTP 200"
jq -r '.assets[] | "\(.name)\t\(.url)"' "$MANIFEST" | while IFS=$'\t' read -r name url; do
  code="$(curl -sSL -o "$ASSETS_DIR/$name" -w '%{http_code}' \
    -H 'Accept: application/octet-stream' \
    -H "Authorization: Bearer $(gh auth token)" \
    "$url")"
  [[ "$code" == "200" ]] || fail "$name: HTTP $code from $url"
done
ok "every asset returned HTTP 200"

step "Locating SHA256SUMS.txt"
SUMS="$ASSETS_DIR/SHA256SUMS.txt"
[[ -f "$SUMS" ]] || fail "SHA256SUMS.txt missing from release"
ok "SHA256SUMS.txt present"

step "Verifying every jarvy* artifact appears in SHA256SUMS.txt"
MISSING=0
while IFS= read -r asset; do
  base="$(basename "$asset")"
  if ! grep -qE "(^|/)${base}\$" "$SUMS"; then
    printf '  \033[1;31m✗\033[0m %s missing from SHA256SUMS.txt\n' "$base" >&2
    MISSING=$((MISSING+1))
  fi
done < <(find "$ASSETS_DIR" -type f -name 'jarvy*' \
           ! -name '*.sig' ! -name '*.pem' ! -name '*.bundle')
[[ "$MISSING" -eq 0 ]] || fail "$MISSING artifact(s) not listed in SHA256SUMS.txt"
ok "every jarvy* artifact listed"

step "Recomputing sha256 for every listed artifact"
# SHA256SUMS.txt format: "<hex>  <filename>" (BSD/GNU compatible).
BAD=0
while IFS= read -r line; do
  expected="${line%% *}"
  file_rel="${line##* }"
  file="$ASSETS_DIR/${file_rel##*/}"
  [[ -f "$file" ]] || { printf '  skip: %s not downloaded\n' "$file_rel"; continue; }
  actual="$(sha256 "$file")"
  if [[ "$expected" != "$actual" ]]; then
    printf '  \033[1;31m✗\033[0m %s expected=%s actual=%s\n' \
      "$file_rel" "$expected" "$actual" >&2
    BAD=$((BAD+1))
  fi
done <"$SUMS"
[[ "$BAD" -eq 0 ]] || fail "$BAD checksum mismatch(es)"
ok "every checksum matches"

step "Verifying Sigstore signatures (cosign verify-blob)"
SIG_BAD=0
while IFS= read -r artifact; do
  bundle="${artifact}.bundle"
  sig="${artifact}.sig"
  pem="${artifact}.pem"
  for f in "$bundle" "$sig" "$pem"; do
    [[ -f "$f" ]] || fail "missing signature file: $(basename "$f")"
  done
  # Keyless verify — identity is GitHub Actions OIDC from the release workflow.
  # `--certificate-identity-regexp` is the canonical pin for "any tag in this repo".
  if ! cosign verify-blob \
        --bundle "$bundle" \
        --certificate-identity-regexp "^https://github.com/${REPO}/\\.github/workflows/release\\.yml@refs/tags/" \
        --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
        "$artifact" >/dev/null 2>&1; then
    printf '  \033[1;31m✗\033[0m %s cosign verify-blob failed\n' "$(basename "$artifact")" >&2
    SIG_BAD=$((SIG_BAD+1))
  fi
done < <(find "$ASSETS_DIR" -type f \( -name 'jarvy*' -o -name 'sbom.*' \) \
           ! -name '*.sig' ! -name '*.pem' ! -name '*.bundle')
[[ "$SIG_BAD" -eq 0 ]] || fail "$SIG_BAD signature verification failure(s)"
ok "every signature verified"

step "Validating SBOMs"
SBOM_COUNT=0
for sbom in "$ASSETS_DIR"/sbom.*; do
  [[ -f "$sbom" ]] || continue
  case "$sbom" in
    *.sig|*.pem|*.bundle) continue ;;
  esac
  # Reject empty / non-JSON. Both CycloneDX and SPDX JSON are valid JSON
  # documents at the top level.
  if ! jq -e '.' "$sbom" >/dev/null 2>&1; then
    fail "$(basename "$sbom") is not valid JSON"
  fi
  # Recognise format by a distinguishing top-level key.
  fmt="$(jq -r '
    if .bomFormat == "CycloneDX" then "CycloneDX"
    elif .spdxVersion then "SPDX"
    else "unknown"
    end' "$sbom")"
  [[ "$fmt" != "unknown" ]] || fail "$(basename "$sbom") format not recognised"
  ok "$(basename "$sbom") parses as $fmt"
  SBOM_COUNT=$((SBOM_COUNT+1))
done
[[ "$SBOM_COUNT" -gt 0 ]] || fail "no SBOM artifacts found"

step "Running --version on platform-matching binary"
# Jarvy ships native installers (.dmg / .deb / .rpm / .AppImage / .msi / .exe),
# not tarballs. Of those, only .deb has a portable, no-privilege extraction
# path (`ar x` + tar) that works on any Linux host, so the --version probe is
# Linux-only. macOS and Windows hosts log a skip — install-path coverage on
# those platforms is exercised by Paths 1-5.
HOST_OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
  arm64|aarch64) DEB_ARCH="arm64"  ;;
  x86_64|amd64)  DEB_ARCH="amd64"  ;;
  armv7l|armhf)  DEB_ARCH="armhf"  ;;
  *)             DEB_ARCH=""       ;;
esac

if [[ "$HOST_OS" != "linux" || -z "$DEB_ARCH" ]]; then
  printf '  \033[1;33m·\033[0m host %s/%s — skipping --version probe (Linux-only)\n' \
    "$HOST_OS" "$HOST_ARCH"
else
  CANDIDATE="$(find "$ASSETS_DIR" -maxdepth 1 -type f -name "jarvy*_${DEB_ARCH}.deb" | head -1)"
  if [[ -z "$CANDIDATE" ]]; then
    printf '  \033[1;33m·\033[0m no .deb matched arch %s — skipping --version probe\n' "$DEB_ARCH"
  else
    require_cmd ar
    EXTRACT="$WORKDIR/extract"
    mkdir -p "$EXTRACT"
    ( cd "$EXTRACT" && ar x "$CANDIDATE" )
    # data.tar may be .gz / .xz / .zst depending on packager defaults.
    DATA_TAR="$(find "$EXTRACT" -maxdepth 1 -name 'data.tar.*' | head -1)"
    [[ -n "$DATA_TAR" ]] || fail "no data.tar.* inside $(basename "$CANDIDATE")"
    mkdir -p "$EXTRACT/data"
    tar -xf "$DATA_TAR" -C "$EXTRACT/data"
    BIN="$(find "$EXTRACT/data" -type f -name 'jarvy' -perm -u+x | head -1)"
    [[ -n "$BIN" ]] || fail "no jarvy binary inside $(basename "$CANDIDATE")"
    reported="$("$BIN" --version 2>&1 | head -1)"
    if [[ "$reported" != *"$VERSION"* ]]; then
      fail "$BIN --version reported '$reported', expected to contain '$VERSION'"
    fi
    ok "$(basename "$CANDIDATE") binary reports: $reported"
  fi
fi

step "Sweep complete"
ok "Path 8 PASS for $TAG"
