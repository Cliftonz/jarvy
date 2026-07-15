#!/usr/bin/env bash
# Unit tests for dist/scripts/install.sh
#
# Network-free: sources install.sh (which no longer auto-runs main() when
# sourced) and exercises the pure helper functions in isolation —
# verify_checksum (the integrity gate), fetch_expected_sha (SHA256SUMS.txt
# parsing, with `curl` stubbed), and matches_channel (stable/beta/nightly
# tag filtering). This is where the tampered-archive negative test and the
# channel-filter branch coverage live; the live end-to-end run is
# .github/workflows/installer-e2e.yml.
#
# Usage: bash dist/scripts/tests/install_sh_test.sh

# JARVY_CHANNEL is consumed by matches_channel() inside the sourced
# install.sh, which shellcheck can't follow — suppress the false "unused".
# shellcheck disable=SC2034
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_SH="$SCRIPT_DIR/../install.sh"

PASS=0
FAIL=0

ok()   { PASS=$((PASS + 1)); printf '  \033[0;32mok\033[0m   %s\n' "$1"; }
bad()  { FAIL=$((FAIL + 1)); printf '  \033[0;31mFAIL\033[0m %s\n' "$1"; }

assert_eq() {
    # assert_eq <label> <expected> <actual>
    if [ "$2" = "$3" ]; then ok "$1"; else bad "$1 (expected '$2', got '$3')"; fi
}
assert_rc() {
    # assert_rc <label> <expected-rc> <actual-rc>
    if [ "$2" = "$3" ]; then ok "$1"; else bad "$1 (expected rc $2, got $3)"; fi
}

# ---------------------------------------------------------------------------
# `curl` stub — fetch_expected_sha shells out to curl. Shadow it with a
# function that returns a canned SHA256SUMS.txt body (note the `./` prefix
# that release.yml emits) so parsing is tested without a network hop.
# ---------------------------------------------------------------------------
CURL_SHOULD_FAIL=0
curl() {
    if [ "$CURL_SHOULD_FAIL" = "1" ]; then return 22; fi
    # Entry shapes cover all manifest generations: bare `./` prefix,
    # BUILD-PATHED (./release/... — pre-2026-07-15 release.yml emitted
    # these and lookups silently missed them), and bare basename (what
    # release.yml emits now).
    cat <<'SUMS'
1111111111111111111111111111111111111111111111111111111111111111  ./jarvy-v1.2.3-x86_64-unknown-linux-musl.tar.gz
2222222222222222222222222222222222222222222222222222222222222222  ./jarvy-v1.2.3-aarch64-apple-darwin.tar.gz
4444444444444444444444444444444444444444444444444444444444444444  ./release/jarvy-v1.2.3-aarch64-unknown-linux-gnu.tar.gz
5555555555555555555555555555555555555555555555555555555555555555  jarvy-v1.2.3-x86_64-pc-windows-msvc.zip
3333333333333333333333333333333333333333333333333333333333333333  sbom.spdx.json
SUMS
}

# Source the installer. BASH_SOURCE guard keeps main() from firing.
# shellcheck disable=SC1090
source "$INSTALL_SH"
# install.sh runs `set -euo pipefail` at the top, which leaks into this
# shell when sourced. The tests deliberately call functions that return
# non-zero (rejected channels, mismatched digests), so drop -e here.
set +e

echo "== matches_channel =="
JARVY_CHANNEL=stable;  matches_channel "v1.2.3";        assert_rc "stable accepts release" 0 $?
JARVY_CHANNEL=stable;  matches_channel "v1.2.3-rc.1";   assert_rc "stable rejects rc"      1 $?
JARVY_CHANNEL=beta;    matches_channel "v1.2.3-rc.1";   assert_rc "beta accepts rc"        0 $?
JARVY_CHANNEL=beta;    matches_channel "v1.2.3-beta.2"; assert_rc "beta accepts beta"      0 $?
JARVY_CHANNEL=beta;    matches_channel "v1.2.3-alpha.1";assert_rc "beta rejects alpha"     1 $?
JARVY_CHANNEL=beta;    matches_channel "v1.2.3";        assert_rc "beta accepts release"   0 $?
JARVY_CHANNEL=nightly; matches_channel "v1.2.3-alpha.9";assert_rc "nightly accepts alpha"  0 $?

echo "== resolve_linux_triple (must match release.yml's shipped assets) =="
# Regression guard for the glibc-x86_64 404: the installer MUST request
# the static musl build on x86_64, gnu on aarch64, gnueabihf on armv7 —
# regardless of the box's actual libc.
assert_eq "x86_64 -> musl"  "x86_64-unknown-linux-musl"      "$(resolve_linux_triple x86_64)"
assert_eq "amd64  -> musl"  "x86_64-unknown-linux-musl"      "$(resolve_linux_triple amd64)"
assert_eq "aarch64 -> gnu"  "aarch64-unknown-linux-gnu"      "$(resolve_linux_triple aarch64)"
assert_eq "arm64  -> gnu"   "aarch64-unknown-linux-gnu"      "$(resolve_linux_triple arm64)"
assert_eq "armv7l -> gnueabihf" "armv7-unknown-linux-gnueabihf" "$(resolve_linux_triple armv7l)"
resolve_linux_triple riscv64 >/dev/null
assert_rc "unsupported arch returns non-zero" 1 $?

echo "== fetch_expected_sha =="
GOT="$(fetch_expected_sha "1.2.3" "jarvy-v1.2.3-aarch64-apple-darwin.tar.gz")"
assert_eq "matches the darwin digest" \
    "2222222222222222222222222222222222222222222222222222222222222222" "$GOT"
GOT="$(fetch_expected_sha "1.2.3" "jarvy-v1.2.3-x86_64-unknown-linux-musl.tar.gz")"
assert_eq "matches the musl digest" \
    "1111111111111111111111111111111111111111111111111111111111111111" "$GOT"
GOT="$(fetch_expected_sha "1.2.3" "jarvy-v1.2.3-aarch64-unknown-linux-gnu.tar.gz")"
assert_eq "matches a BUILD-PATHED entry by basename (installer-e2e first-run regression)" \
    "4444444444444444444444444444444444444444444444444444444444444444" "$GOT"
GOT="$(fetch_expected_sha "1.2.3" "jarvy-v1.2.3-x86_64-pc-windows-msvc.zip")"
assert_eq "matches a bare-basename entry" \
    "5555555555555555555555555555555555555555555555555555555555555555" "$GOT"
fetch_expected_sha "1.2.3" "jarvy-v1.2.3-nonexistent.tar.gz" >/dev/null
assert_rc "unlisted archive returns non-zero" 1 $?
CURL_SHOULD_FAIL=1
fetch_expected_sha "1.2.3" "jarvy-v1.2.3-x86_64-unknown-linux-musl.tar.gz" >/dev/null
assert_rc "unreachable sums file returns non-zero" 1 $?
CURL_SHOULD_FAIL=0

echo "== verify_checksum (integrity gate) =="
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
printf 'jarvy-payload' > "$TMP"
if command -v sha256sum >/dev/null 2>&1; then
    REAL_SHA="$(sha256sum "$TMP" | awk '{print $1}')"
else
    REAL_SHA="$(shasum -a 256 "$TMP" | awk '{print $1}')"
fi
verify_checksum "$TMP" "$REAL_SHA" >/dev/null 2>&1
assert_rc "accepts a matching digest" 0 $?
# The tampered-archive negative test: a wrong digest MUST be rejected.
verify_checksum "$TMP" "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef00" >/dev/null 2>&1
assert_rc "rejects a mismatched digest" 1 $?

echo ""
echo "install.sh unit tests: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
