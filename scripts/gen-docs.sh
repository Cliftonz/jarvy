#!/usr/bin/env bash
# Generate auto-built docs pages from the jarvy binary.
#
# Outputs:
#   docs/cli-reference.md     — full `jarvy --help` tree
#   docs/tools-registry.md    — every tool in the registry, formatted
#
# Usage:
#   cargo build --bin jarvy
#   bash scripts/gen-docs.sh
#
# Run before `mkdocs build` in CI. The generated files are checked into git
# so the docs site builds without needing the binary.

set -euo pipefail

# Defense in depth: silence the tracing console layer so an info-level
# event from jarvy startup cannot contaminate a JSON-emitting command's
# stdout. The architectural fix lives in src/analytics.rs (non-error
# console logs go to stderr), but RUST_LOG=off here makes this script
# robust against future regressions in either direction.
export RUST_LOG="${RUST_LOG:-off}"
export JARVY_TELEMETRY=0

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

JARVY_BIN="${JARVY_BIN:-$REPO_ROOT/target/debug/jarvy}"
if [[ ! -x "$JARVY_BIN" ]]; then
    JARVY_BIN="$REPO_ROOT/target/release/jarvy"
fi
if [[ ! -x "$JARVY_BIN" ]]; then
    echo "error: jarvy binary not found at target/debug/jarvy or target/release/jarvy" >&2
    echo "       run 'cargo build --bin jarvy' first" >&2
    exit 1
fi

# Strip ANSI color codes so the output is plain markdown-friendly text.
strip_ansi() {
    sed -E 's/\x1b\[[0-9;]*[a-zA-Z]//g'
}

# ----------------------------------------------------------------------
# 1. CLI reference
# ----------------------------------------------------------------------
out="docs/cli-reference.md"
{
    cat <<'EOF'
---
title: "CLI reference (auto-generated) — Jarvy"
description: "Complete jarvy command-line reference, generated from the binary's --help output. Always reflects the latest version."
tags:
  - reference
---

# CLI reference

!!! info "Auto-generated"
    This page is generated from `jarvy --help` by `scripts/gen-docs.sh`. To
    update it, run that script after a `cargo build`. Anything you write
    here by hand will be overwritten on the next regeneration.

## `jarvy`

```text
EOF
    "$JARVY_BIN" --help 2>&1 | strip_ansi
    echo '```'
    echo
    echo '## Subcommands'
    echo

    # Enumerate subcommands by parsing the top-level --help.
    subcmds=$(
        "$JARVY_BIN" --help 2>&1 \
        | strip_ansi \
        | awk '/^Commands:/{flag=1; next} /^[A-Z][a-z]/{flag=0} flag && /^  [a-z]/{print $1}'
    )
    for sub in $subcmds; do
        echo "### \`jarvy $sub\`"
        echo
        echo '```text'
        "$JARVY_BIN" "$sub" --help 2>&1 | strip_ansi || true
        echo '```'
        echo
    done
} > "$out"
echo "wrote $out ($(wc -l < "$out") lines)"

# ----------------------------------------------------------------------
# 2. Tool registry
# ----------------------------------------------------------------------
out="docs/tools-registry.md"
"$JARVY_BIN" tools --index --format json > /tmp/jarvy-tools-index.json

# Diagnostic: surface the first 3 lines + total byte count so any future
# stdout contamination is visible in CI logs without needing to reproduce
# the failure locally. Non-fatal — proceeds to the Python parse below.
echo "--- /tmp/jarvy-tools-index.json head (3 lines, $(wc -c < /tmp/jarvy-tools-index.json) bytes) ---"
head -3 /tmp/jarvy-tools-index.json
echo "--- end head ---"

python3 - "$out" <<'PY'
import json, sys
from pathlib import Path

out_path = Path(sys.argv[1])
data = json.loads(Path("/tmp/jarvy-tools-index.json").read_text())
tools = data.get("tools", [])
count = data.get("count", len(tools))

lines = [
    "---",
    'title: "Tool registry (auto-generated) — Jarvy"',
    f'description: "Every tool Jarvy knows how to install — {count} entries spanning runtimes, build tools, cloud SDKs, container tools, security scanners, and editors."',
    "tags:",
    "  - reference",
    "  - tools",
    "---",
    "",
    "# Tool registry",
    "",
    "!!! info \"Auto-generated\"",
    "    This page is generated from `jarvy tools --index` by `scripts/gen-docs.sh`. ",
    "    Run that script after registering new tools.",
    "",
    f"Jarvy currently ships **{count} tools**. Reference one in your `jarvy.toml` by its **name**.",
    "",
    "| Name | Command | macOS | Linux | Windows | Default hook | Depends on |",
    "|---|---|---|---|---|---|---|",
]

def fmt_macos(m):
    if not m: return "—"
    parts = []
    if m.get("brew"): parts.append(f"`brew: {m['brew']}`")
    if m.get("cask"): parts.append(f"`cask: {m['cask']}`")
    if m.get("custom_install"): parts.append("custom")
    return ", ".join(parts) or "—"

def fmt_linux(l):
    if not l: return "—"
    if l.get("uniform"): return f"`{l['uniform']}`"
    parts = []
    for mgr in ("apt", "dnf", "pacman", "apk"):
        if l.get(mgr): parts.append(f"{mgr}: `{l[mgr]}`")
    if l.get("custom_install"): parts.append("custom")
    return "<br>".join(parts) or "—"

def fmt_windows(w):
    if not w: return "—"
    parts = []
    if w.get("winget"): parts.append(f"`winget: {w['winget']}`")
    if w.get("choco"): parts.append(f"`choco: {w['choco']}`")
    if w.get("scoop"): parts.append(f"`scoop: {w['scoop']}`")
    if w.get("custom_install"): parts.append("custom")
    return "<br>".join(parts) or "—"

for t in sorted(tools, key=lambda x: x["name"]):
    name = t["name"]
    cmd  = f"`{t.get('command', name)}`"
    macos = fmt_macos(t.get("macos"))
    linux = fmt_linux(t.get("linux"))
    windows = fmt_windows(t.get("windows"))
    has_hook = "✓" if t.get("default_hook") else ""
    deps = t.get("depends_on") or t.get("depends_on_one_of") or []
    deps_str = ", ".join(f"`{d}`" for d in deps[:3]) + ("…" if len(deps) > 3 else "") if deps else "—"
    lines.append(f"| `{name}` | {cmd} | {macos} | {linux} | {windows} | {has_hook} | {deps_str} |")

lines.append("")
lines.append("---")
lines.append("")
lines.append("## Don't see what you need?")
lines.append("")
lines.append("- Try a fuzzy search: `jarvy search <name>`")
lines.append("- Add a new tool — see [Adding tools](adding-tools.md) for the macro and PR flow.")
lines.append("")

out_path.write_text("\n".join(lines))
print(f"wrote {out_path} ({len(lines)} lines, {count} tools)")
PY

rm -f /tmp/jarvy-tools-index.json

echo ""
echo "Done. Run \`mkdocs build --strict\` to verify."
