#!/usr/bin/env bash
# Dogfood the PRD-058 agent profile switcher against REAL agent config
# data, without touching the real dirs.
#
# The e2e test seeds synthetic files. This seeds a sandbox from the
# operator's actual ~/.claude / ~/.codex / ~/.cursor identity surface
# (settings, auth, config, skills, MCP registrations) so the lifecycle
# runs over real shapes: real JSON, real modes, real symlinks, real
# unicode filenames. Everything is copied OUT of the real dirs; the
# originals are opened read-only and never moved, linked, or deleted.
#
# Usage:
#   cargo build --bin jarvy
#   bash scripts/dogfood-agent-profiles.sh [--keep]
#
#   --keep   leave the sandbox on disk for inspection
#
# Exit non-zero on the first violated invariant.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

JARVY_BIN="${JARVY_BIN:-$REPO_ROOT/target/debug/jarvy}"
[[ -x "$JARVY_BIN" ]] || JARVY_BIN="$REPO_ROOT/target/release/jarvy"
if [[ ! -x "$JARVY_BIN" ]]; then
    echo "error: jarvy binary not found; run 'cargo build --bin jarvy'" >&2
    exit 1
fi

KEEP=0
[[ "${1:-}" == "--keep" ]] && KEEP=1

SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/jarvy-profile-dogfood.XXXXXX")"
cleanup() {
    if [[ $KEEP -eq 1 ]]; then
        echo "sandbox kept: $SANDBOX"
    else
        chmod -R u+w "$SANDBOX" 2>/dev/null || true
        rm -rf "$SANDBOX"
    fi
}
trap cleanup EXIT

# JARVY_HOME is an always-compiled override (not test-bypass gated) and
# acts as BOTH the .jarvy store root and the agent $HOME, so the whole
# switcher runs hermetically inside the sandbox.
jarvy_p() {
    env -u CLAUDE_CONFIG_DIR -u CODEX_HOME \
        JARVY_HOME="$SANDBOX" \
        JARVY_TEST_MODE=1 \
        JARVY_TELEMETRY=0 \
        JARVY_NO_PERSONAL_CONFIG=1 \
        RUST_LOG=off \
        "$JARVY_BIN" agents profile "$@"
}

# BSD stat (macOS) and GNU stat (Linux) disagree on the mode flag.
fmode() { stat -f '%Lp' "$1" 2>/dev/null || stat -c '%a' "$1" 2>/dev/null; }

PASS=0
FAIL=0
ok()   { PASS=$((PASS+1)); printf '  \033[32mok\033[0m   %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); printf '  \033[31mFAIL\033[0m %s\n' "$1"; }
step() { printf '\n\033[1m%s\033[0m\n' "$1"; }

# ----------------------------------------------------------------------
# Seed the sandbox from the real identity surface (read-only source).
# ----------------------------------------------------------------------
step "seed sandbox from real agent dirs"

seed_one() {
    local real="$1" slug="$2"; shift 2
    local dest="$SANDBOX/$slug"
    if [[ ! -d "$real" ]]; then
        echo "  skip $slug (no $real)"
        return
    fi
    mkdir -p "$dest"
    local copied=0
    for item in "$@"; do
        if [[ -e "$real/$item" ]]; then
            cp -R "$real/$item" "$dest/$item" 2>/dev/null || continue
            copied=$((copied+1))
        fi
    done
    printf '  %-10s seeded %d items (%s)\n' "$slug" "$copied" "$(du -sh "$dest" | cut -f1)"
}

# Identity surface only. Deliberately excludes the bulk that dominates a
# real dir (projects/, packages/, extensions/, *.sqlite, caches) — see
# the size report at the end for why that distinction matters.
seed_one "$HOME/.claude"  ".claude" \
    settings.json .credentials.json CLAUDE.md skills agents commands mcp_servers.json
seed_one "$HOME/.codex"   ".codex" \
    auth.json config.toml skills AGENTS.md prompts
seed_one "$HOME/.cursor"  ".cursor" \
    mcp.json cli-config.json skills-cursor rules

# ----------------------------------------------------------------------
step "init — snapshot live config as 'default'"
# ----------------------------------------------------------------------
t0=$(date +%s)
jarvy_p init >/dev/null
t1=$(date +%s)
echo "  init took $((t1-t0))s"

REG="$SANDBOX/agent-profiles/profiles.json"
[[ -f "$REG" ]] && ok "registry written" || bad "registry missing at $REG"

mode=$(fmode "$SANDBOX/agent-profiles")
[[ "$mode" == "700" ]] && ok "store dir is 0700" || bad "store dir mode $mode, want 700"

# Credential files must keep their original restrictive mode, not inherit
# the copier's umask.
for cred in "$SANDBOX/agent-profiles/default/codex/auth.json" \
            "$SANDBOX/agent-profiles/default/claude-code/.credentials.json"; do
    if [[ -f "$cred" ]]; then
        src="$HOME/.codex/auth.json"
        [[ "$cred" == *claude-code* ]] && src="$HOME/.claude/.credentials.json"
        want=$(fmode "$src")
        got=$(fmode "$cred")
        [[ "$want" == "$got" ]] \
            && ok "$(basename "$cred") preserved mode $got" \
            || bad "$(basename "$cred") mode $got, source was $want"
    fi
done

# ----------------------------------------------------------------------
step "cursor — symlink tier replaced the live dir"
# ----------------------------------------------------------------------
if [[ -e "$SANDBOX/.cursor" ]]; then
    if [[ -L "$SANDBOX/.cursor" ]]; then
        tgt=$(readlink "$SANDBOX/.cursor")
        ok "~/.cursor is a symlink -> $tgt"
        [[ -f "$SANDBOX/.cursor/mcp.json" || -d "$SANDBOX/.cursor/skills-cursor" ]] \
            && ok "content readable through the link" \
            || bad "link resolves but content missing"
    else
        bad "~/.cursor is not a symlink after init (move tier did not fire)"
    fi
fi

# ----------------------------------------------------------------------
step "create --from + list"
# ----------------------------------------------------------------------
jarvy_p create work --from default >/dev/null
listed=$(jarvy_p list --format json)
echo "$listed" | grep -q '"work"' && ok "work appears in list --format json" \
    || bad "work missing from list json"
echo "$listed" | python3 -m json.tool >/dev/null 2>&1 \
    && ok "list --format json is valid JSON" || bad "list json is malformed"

# ----------------------------------------------------------------------
step "use --print-env — stdout purity + shell parseability"
# ----------------------------------------------------------------------
envout=$(jarvy_p use work --print-env 2>/dev/null)
echo "$envout"| sed 's/^/    | /'

if [[ -n "$envout" ]]; then
    # Every stdout line must be an export line. Anything else means human
    # text leaked into the stream `jp` feeds to eval.
    if [[ -z "$(echo "$envout" | grep -v '^export [A-Z_]*="' || true)" ]]; then
        ok "stdout carries only export lines"
    else
        bad "non-export content on stdout (would break 'eval')"
    fi
    for sh in bash zsh; do
        if command -v $sh >/dev/null; then
            echo "$envout" | $sh -n 2>/dev/null \
                && ok "$sh parses the export block" \
                || bad "$sh cannot parse the export block"
        fi
    done
    # The exported dir must actually exist, or the agent silently starts
    # with an empty config.
    while IFS= read -r line; do
        var=${line%%=*}; var=${var#export }
        val=${line#*=}; val=${val%\"}; val=${val#\"}
        [[ -d "$val" ]] && ok "$var points at an existing dir" \
            || bad "$var points at missing dir: $val"
    done <<< "$envout"
else
    bad "use --print-env produced no output"
fi

# ----------------------------------------------------------------------
step "shell-init jp snippet parses in each shell"
# ----------------------------------------------------------------------
for sh in bash zsh; do
    if command -v $sh >/dev/null; then
        snip=$(env JARVY_HOME="$SANDBOX" JARVY_TELEMETRY=0 RUST_LOG=off \
            "$JARVY_BIN" shell-init --shell $sh 2>/dev/null)
        echo "$snip" | grep -q 'jp' && ok "$sh snippet defines jp" \
            || bad "$sh snippet missing jp"
        echo "$snip" | $sh -n 2>/dev/null && ok "$sh snippet is syntactically valid" \
            || bad "$sh snippet fails '$sh -n'"
    fi
done

# ----------------------------------------------------------------------
step "refusals"
# ----------------------------------------------------------------------
jarvy_p create ../escape >/dev/null 2>&1 \
    && bad "traversal name accepted" || ok "traversal profile name refused"
jarvy_p use nonexistent-profile >/dev/null 2>&1 \
    && bad "unknown profile accepted" || ok "unknown profile refused"
jarvy_p delete default >/dev/null 2>&1 \
    && bad "deleted the active profile" || ok "active profile delete refused"

# ----------------------------------------------------------------------
step "status --format json"
# ----------------------------------------------------------------------
st=$(jarvy_p status --format json)
echo "$st" | python3 -m json.tool >/dev/null 2>&1 \
    && ok "status json is valid" || bad "status json malformed"
echo "$st" | grep -q 'claude-code' && ok "status covers claude-code" \
    || bad "status missing claude-code"

# ----------------------------------------------------------------------
step "delete non-active profile is clean"
# ----------------------------------------------------------------------
jarvy_p create scratch >/dev/null 2>&1 || true
jarvy_p delete scratch >/dev/null 2>&1 \
    && ok "non-active profile deleted" || bad "could not delete scratch profile"
[[ ! -d "$SANDBOX/agent-profiles/scratch" ]] \
    && ok "scratch dir removed from disk" || bad "scratch dir survived delete"

# ----------------------------------------------------------------------
step "real-world cost projection (what init would copy on this machine)"
# ----------------------------------------------------------------------
total=0
for d in "$HOME/.claude" "$HOME/.codex" "$HOME/.cursor"; do
    if [[ -d "$d" ]]; then
        kb=$(du -sk "$d" 2>/dev/null | cut -f1)
        total=$((total + kb))
        printf '  %-22s %s\n' "$(basename "$d")" "$(du -sh "$d" 2>/dev/null | cut -f1)"
    fi
done
seeded=$(du -sk "$SANDBOX" 2>/dev/null | cut -f1)
printf '  %-22s %s MB\n' "TOTAL per profile" "$((total / 1024))"
printf '  %-22s %s MB\n' "identity surface only" "$((seeded / 1024))"
if [[ $seeded -gt 0 ]]; then
    printf '  %-22s %sx\n' "copied / actually-needed" "$((total / seeded))"
fi

# ----------------------------------------------------------------------
printf '\n\033[1mresult:\033[0m %d passed, %d failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
