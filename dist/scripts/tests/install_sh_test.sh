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

echo "== main() install target handling =="
# Runs the whole installer as a subprocess with a PATH-stubbed `curl`
# that serves a fake tarball, so the swap, trap, and target checks are
# exercised without a network hop. Expects a Linux or macOS host.
WORK="$(mktemp -d)"
trap 'rm -rf "$TMP" "$WORK"' EXIT
mkdir -p "$WORK/pkg" "$WORK/stub"
printf '#!/bin/sh\necho "jarvy 1.2.3"\n' > "$WORK/pkg/jarvy"
chmod 755 "$WORK/pkg/jarvy"
tar -C "$WORK/pkg" -czf "$WORK/jarvy.tar.gz" jarvy
cat > "$WORK/stub/curl" <<EOF
#!/bin/sh
touch "$WORK/curl-called"
while [ \$# -gt 0 ]; do
    if [ "\$1" = "-o" ]; then cp "$WORK/jarvy.tar.gz" "\$2"; exit 0; fi
    shift
done
exit 1
EOF
chmod 755 "$WORK/stub/curl"

run_install() {
    # run_install <install-dir> [extra PATH dir]; sets RC and OUT
    rm -f "$WORK/curl-called"
    rm -rf "$WORK/tmp"
    mkdir -p "$WORK/tmp"
    OUT="$(PATH="${2:+$2:}$WORK/stub:$PATH" TMPDIR="$WORK/tmp" \
        JARVY_VERSION=1.2.3 JARVY_SKIP_CHECKSUM=1 JARVY_NO_MODIFY_PATH=1 \
        JARVY_INSTALL_DIR="$1" bash "$INSTALL_SH" 2>&1)"
    RC=$?
}
leftovers() { find "$1" -name '.jarvy.*' 2>/dev/null; ls -A "$WORK/tmp"; }

DIR="$WORK/o'neil/bin"
run_install "$DIR"
assert_rc "apostrophe in install dir installs" 0 "$RC"
assert_eq "apostrophe in install dir leaves nothing behind" "" "$(leftovers "$DIR")"

DIR="$WORK/linked/bin"
mkdir -p "$DIR"
touch "$WORK/elsewhere"
ln -s "$WORK/elsewhere" "$DIR/jarvy"
run_install "$DIR"
assert_rc "live symlink target is refused" 1 "$RC"
assert_eq "live symlink refusal names the cause" "1" "$(printf '%s' "$OUT" | grep -c 'is a symlink')"
assert_eq "live symlink refused before any download" "no" "$([ -e "$WORK/curl-called" ] && echo yes || echo no)"
assert_eq "live symlink left intact" "$WORK/elsewhere" "$(readlink "$DIR/jarvy")"

DIR="$WORK/dangling/bin"
mkdir -p "$DIR"
ln -s "$WORK/does-not-exist" "$DIR/jarvy"
run_install "$DIR"
assert_rc "dangling symlink target is replaced" 0 "$RC"
assert_eq "dangling symlink replaced by a 0755 regular file" "-rwxr-xr-x" "$(ls -l "$DIR/jarvy" | cut -c1-10)"

DIR="$WORK/dirtarget/bin"
mkdir -p "$DIR/jarvy"
run_install "$DIR"
assert_rc "directory target is refused" 1 "$RC"
assert_eq "directory refusal names the cause" "1" "$(printf '%s' "$OUT" | grep -c 'is a directory')"
assert_eq "directory target refused before any download" "no" "$([ -e "$WORK/curl-called" ] && echo yes || echo no)"

mkdir -p "$WORK/failing-chmod"
printf '#!/bin/sh\necho "stub chmod failed" >&2\nexit 1\n' > "$WORK/failing-chmod/chmod"
chmod 755 "$WORK/failing-chmod/chmod"
DIR="$WORK/chmodfail/bin"
run_install "$DIR" "$WORK/failing-chmod"
assert_rc "failing chmod fails the install" 1 "$RC"
assert_eq "failing chmod error is shown" "1" "$(printf '%s' "$OUT" | grep -c 'stub chmod failed')"
assert_eq "failing chmod leaves nothing behind" "" "$(leftovers "$DIR")"

# The stub interrupts the installer, then dies of SIGINT itself so bash
# treats the interrupt as unhandled and exits instead of continuing.
mkdir -p "$WORK/sigint-chmod"
printf '#!/bin/sh\nkill -INT "$PPID"\nkill -INT $$\n' > "$WORK/sigint-chmod/chmod"
chmod 755 "$WORK/sigint-chmod/chmod"
DIR="$WORK/sigint/bin"
run_install "$DIR" "$WORK/sigint-chmod"
assert_rc "SIGINT mid-install exits 130" 130 "$RC"
assert_eq "SIGINT mid-install leaves nothing behind" "" "$(leftovers "$DIR")"

if command -v dash >/dev/null 2>&1; then
    OUT="$(dash "$INSTALL_SH" 2>&1)"
    assert_rc "dash run exits non-zero" 1 $?
    assert_eq "dash run gets the bash guard message" "1" "$(printf '%s' "$OUT" | grep -c 'requires bash')"
else
    echo "  skip dash not installed"
fi

echo ""
echo "install.sh unit tests: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
