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

# ----------------------------------------------------------------------
# 3. Searchable tool directory
# ----------------------------------------------------------------------
# Interactive page: search-as-you-type, OS + category filters, and the
# actual install command per package manager for every tool. The data is
# the same `jarvy tools --index` JSON, embedded inline so the page is
# self-contained (no fetch, works on GitHub Pages). The static
# tools-registry.md table above stays as the crawler/noscript-friendly
# view; this page is the human one.
out="docs/tools-directory.md"
python3 - "$out" <<'PY'
import json, sys
from pathlib import Path

out_path = Path(sys.argv[1])
data = json.loads(Path("/tmp/jarvy-tools-index.json").read_text())
tools = data.get("tools", [])
count = data.get("count", len(tools))

# Slim the payload: drop yum/zypper (mirror dnf), drop nulls.
slim = []
for t in sorted(tools, key=lambda x: x["name"]):
    e = {"n": t["name"], "c": t.get("command", t["name"])}
    if t.get("category"):
        e["cat"] = t["category"]
    m = t.get("macos") or {}
    if m.get("brew"): e["brew"] = m["brew"]
    if m.get("cask"): e["cask"] = m["cask"]
    l = t.get("linux") or {}
    for k in ("apt", "dnf", "pacman", "apk"):
        if l.get(k): e[k] = l[k]
    if l.get("brew"): e["lbrew"] = l["brew"]
    w = t.get("windows") or {}
    if w.get("winget"): e["winget"] = w["winget"]
    if w.get("choco"): e["choco"] = w["choco"]
    b = t.get("bsd") or {}
    if b.get("pkg"): e["pkg"] = b["pkg"]
    if (t.get("custom_install") or {}).get("has_custom_installer"):
        e["custom"] = True
    deps = t.get("depends_on")
    if deps: e["deps"] = deps
    flex = t.get("depends_on_one_of")
    if flex: e["flex"] = flex
    hook = t.get("default_hook")
    if hook: e["hook"] = hook.get("description", "")
    slim.append(e)

# `</` must not terminate the inline <script> block early.
payload = json.dumps(slim, separators=(",", ":")).replace("</", "<\\/")

head = f"""---
title: "Tool directory — search {count} tools Jarvy can install"
description: "Search every tool Jarvy installs and see the exact install command for macOS (brew), Linux (apt, dnf, pacman, apk), Windows (winget, choco), and BSD."
hide:
  - toc
tags:
  - reference
  - tools
---

# Tool directory

!!! info "Auto-generated"
    This page is generated from `jarvy tools --index` by `scripts/gen-docs.sh`
    and rebuilt on every docs deploy. Do not edit by hand.

Jarvy installs **{count} tools** with one `jarvy setup`. Search below, or use
`jarvy search <name>` from the CLI. Prefer a plain table? See the
[tool registry](tools-registry.md).

And that's just the built-ins — Jarvy also installs any
[npm, pip, cargo, nuget, gem, or go package](packages.md), plus custom tools
via the [plugin registry](registry-remote.md).

<noscript>This directory needs JavaScript — use the
<a href="https://jarvy.dev/tools-registry/">static tool registry table</a> instead.</noscript>
"""

html = r"""
<div id="jt-app">
  <div class="jt-controls">
    <input id="jt-search" type="search" placeholder="Search tools, commands, or package names…"
           autocomplete="off" spellcheck="false" aria-label="Search tools">
    <select id="jt-os" aria-label="Filter by operating system">
      <option value="all">Any OS</option>
      <option value="macos">macOS</option>
      <option value="linux">Linux</option>
      <option value="windows">Windows</option>
      <option value="bsd">BSD</option>
    </select>
    <div id="jt-cats" role="group" aria-label="Filter by category"></div>
  </div>
  <p id="jt-count" aria-live="polite"></p>
  <div id="jt-list"></div>
</div>

<style>
#jt-app { margin-top: .5rem; }
.jt-controls { display: flex; flex-wrap: wrap; gap: .5rem; align-items: center; }
#jt-search {
  flex: 1 1 16rem; padding: .55rem .8rem; font-size: .8rem;
  border: 1px solid var(--md-default-fg-color--lighter);
  border-radius: .2rem; background: var(--md-default-bg-color);
  color: var(--md-default-fg-color);
}
#jt-search:focus { outline: none; border-color: var(--md-accent-fg-color); }
#jt-os {
  padding: .5rem .6rem; font-size: .75rem; border-radius: .2rem;
  border: 1px solid var(--md-default-fg-color--lighter);
  background: var(--md-default-bg-color); color: var(--md-default-fg-color);
}
#jt-cats { display: flex; flex-wrap: wrap; gap: .3rem; }
#jt-cats button {
  padding: .25rem .6rem; font-size: .7rem; border-radius: 1rem; cursor: pointer;
  border: 1px solid var(--md-default-fg-color--lighter);
  background: transparent; color: var(--md-default-fg-color--light);
}
#jt-cats button.jt-on {
  background: var(--md-accent-fg-color); border-color: var(--md-accent-fg-color);
  color: var(--md-accent-bg-color);
}
#jt-count { font-size: .7rem; color: var(--md-default-fg-color--light); margin: .6rem 0 .4rem; }
/* Neutralize Material's admonition-style <details> theming (icon,
   primary-color border, shadow) — these are plain cards. */
.md-typeset .jt-card {
  border: 1px solid var(--md-default-fg-color--lightest);
  border-radius: .2rem; margin: 0 0 .4rem; overflow: hidden;
  box-shadow: none; background: none; font-size: inherit;
}
.md-typeset .jt-card > summary {
  display: flex; flex-wrap: wrap; gap: .5rem; align-items: center;
  padding: .5rem 2rem .5rem .8rem; cursor: pointer; list-style: none;
  background: none; border: none;
}
.md-typeset .jt-card > summary::before { display: none; }
.md-typeset .jt-card > summary::-webkit-details-marker { display: none; }
.md-typeset .jt-card > summary:hover { background: var(--md-code-bg-color); }
.jt-name { font-family: var(--md-code-font-family, monospace); font-weight: 700; font-size: .8rem; }
.jt-badges { display: flex; gap: .25rem; margin-left: auto; }
.jt-badge, .jt-cat {
  font-size: .6rem; padding: .1rem .45rem; border-radius: 1rem;
  border: 1px solid var(--md-default-fg-color--lighter);
  color: var(--md-default-fg-color--light); white-space: nowrap;
}
.jt-cat { border-style: dashed; }
.jt-body { padding: .2rem .8rem .8rem; border-top: 1px solid var(--md-default-fg-color--lightest); }
.jt-body h5 {
  margin: .8rem 0 .25rem; font-size: .65rem; text-transform: uppercase;
  letter-spacing: .05em; color: var(--md-default-fg-color--light);
}
.jt-cmd {
  display: flex; align-items: center; gap: .5rem;
  background: var(--md-code-bg-color); border-radius: .15rem;
  padding: .35rem .6rem; margin: .2rem 0;
}
.jt-cmd code {
  flex: 1; background: none; padding: 0; font-size: .7rem;
  overflow-x: auto; white-space: pre;
}
.jt-cmd .jt-pm { font-size: .6rem; color: var(--md-default-fg-color--light); min-width: 4.5rem; }
.jt-copy {
  border: none; background: none; cursor: pointer; font-size: .7rem;
  color: var(--md-default-fg-color--light); padding: 0 .2rem;
}
.jt-copy:hover { color: var(--md-accent-fg-color); }
.jt-note { font-size: .7rem; color: var(--md-default-fg-color--light); margin: .3rem 0; }
.jt-empty { padding: 1.5rem; text-align: center; color: var(--md-default-fg-color--light); font-size: .8rem; }
</style>

<script id="jarvy-tools-data" type="application/json">__PAYLOAD__</script>

<script>
(function () {
  "use strict";
  const tools = JSON.parse(document.getElementById("jarvy-tools-data").textContent);
  const list = document.getElementById("jt-list");
  const countEl = document.getElementById("jt-count");
  const searchEl = document.getElementById("jt-search");
  const osEl = document.getElementById("jt-os");
  const catsEl = document.getElementById("jt-cats");

  const esc = (s) => String(s).replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

  const supports = (t, os) => {
    if (os === "all" || t.custom) return true;
    if (os === "macos") return !!(t.brew || t.cask);
    if (os === "linux") return !!(t.apt || t.dnf || t.pacman || t.apk || t.lbrew);
    if (os === "windows") return !!(t.winget || t.choco);
    if (os === "bsd") return !!t.pkg;
    return true;
  };

  const cmdRow = (pm, cmd) =>
    '<div class="jt-cmd"><span class="jt-pm">' + esc(pm) + "</span><code>" + esc(cmd) +
    '</code><button class="jt-copy" data-cmd="' + esc(cmd) + '" title="Copy" aria-label="Copy command">⧉</button></div>';

  const section = (title, rows) => rows.length ? "<h5>" + esc(title) + "</h5>" + rows.join("") : "";

  function body(t) {
    let h = section("With Jarvy — any OS", [
      cmdRow("jarvy.toml", '[provisioner]\n' + t.n + ' = "latest"'),
      cmdRow("then run", "jarvy setup"),
    ]);
    const mac = [];
    if (t.cask) mac.push(cmdRow("brew cask", "brew install --cask " + t.cask));
    if (t.brew) mac.push(cmdRow("brew", "brew install " + t.brew));
    h += section("macOS", mac);
    const lin = [];
    if (t.apt) lin.push(cmdRow("apt", "sudo apt install " + t.apt));
    if (t.dnf) lin.push(cmdRow("dnf", "sudo dnf install " + t.dnf));
    if (t.pacman) lin.push(cmdRow("pacman", "sudo pacman -S " + t.pacman));
    if (t.apk) lin.push(cmdRow("apk", "sudo apk add " + t.apk));
    if (t.lbrew) lin.push(cmdRow("linuxbrew", "brew install " + t.lbrew));
    h += section("Linux", lin);
    const win = [];
    if (t.winget) win.push(cmdRow("winget", "winget install -e --id " + t.winget));
    if (t.choco) win.push(cmdRow("choco", "choco install -y " + t.choco));
    h += section("Windows", win);
    if (t.pkg) h += section("BSD", [cmdRow("pkg", "sudo pkg install " + t.pkg)]);
    if (t.custom)
      h += '<p class="jt-note">⚙ Uses a custom installer — Jarvy runs the official install script for you during <code>jarvy setup</code>.</p>';
    if (t.deps)
      h += '<p class="jt-note">Requires: ' + t.deps.map(esc).join(", ") + " (Jarvy installs dependencies first)</p>";
    if (t.flex)
      h += '<p class="jt-note">Works with one of: ' + t.flex.map(esc).join(", ") + "</p>";
    if (t.hook)
      h += '<p class="jt-note">Post-install hook: ' + esc(t.hook) + "</p>";
    return h;
  }

  const osBadges = (t) => {
    const b = [];
    if (t.custom) b.push("custom");
    if (t.brew || t.cask) b.push("macOS");
    if (t.apt || t.dnf || t.pacman || t.apk || t.lbrew) b.push("Linux");
    if (t.winget || t.choco) b.push("Windows");
    if (t.pkg) b.push("BSD");
    return b.map((x) => '<span class="jt-badge">' + x + "</span>").join("");
  };

  // Build all cards once; filtering toggles visibility.
  const frag = document.createDocumentFragment();
  const cards = tools.map((t) => {
    const d = document.createElement("details");
    d.className = "jt-card";
    d.innerHTML =
      "<summary><span class=\"jt-name\">" + esc(t.n) + "</span>" +
      (t.cat ? '<span class="jt-cat">' + esc(t.cat) + "</span>" : "") +
      '<span class="jt-badges">' + osBadges(t) + "</span></summary>" +
      '<div class="jt-body">' + body(t) + "</div>";
    frag.appendChild(d);
    const hay = [t.n, t.c, t.cat, t.brew, t.cask, t.apt, t.dnf, t.pacman, t.apk,
                 t.lbrew, t.winget, t.choco, t.pkg].filter(Boolean).join(" ").toLowerCase();
    return { t, el: d, hay };
  });
  list.appendChild(frag);
  const empty = document.createElement("p");
  empty.className = "jt-empty";
  empty.hidden = true;
  empty.innerHTML = "No tools match. Try <code>jarvy search</code> or " +
    '<a href="https://github.com/Cliftonz/jarvy/issues">request a tool</a>.';
  list.appendChild(empty);

  // Category chips (only categories that exist in the data).
  const cats = [...new Set(tools.map((t) => t.cat).filter(Boolean))].sort();
  let activeCat = "all";
  const chips = ["all", ...cats].map((c) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.textContent = c === "all" ? "All" : c;
    btn.className = c === "all" ? "jt-on" : "";
    btn.addEventListener("click", () => {
      activeCat = c;
      chips.forEach((x) => x.classList.remove("jt-on"));
      btn.classList.add("jt-on");
      apply();
    });
    catsEl.appendChild(btn);
    return btn;
  });

  function apply() {
    const q = searchEl.value.trim().toLowerCase();
    const os = osEl.value;
    let shown = 0;
    for (const { t, el, hay } of cards) {
      const ok =
        (!q || hay.includes(q)) &&
        supports(t, os) &&
        (activeCat === "all" || t.cat === activeCat);
      el.style.display = ok ? "" : "none";
      if (ok) shown++;
    }
    empty.hidden = shown !== 0;
    countEl.textContent = "Showing " + shown + " of " + tools.length + " tools";
  }

  searchEl.addEventListener("input", apply);
  osEl.addEventListener("change", apply);
  list.addEventListener("click", (ev) => {
    const btn = ev.target.closest(".jt-copy");
    if (!btn) return;
    ev.preventDefault();
    navigator.clipboard.writeText(btn.dataset.cmd).then(() => {
      const old = btn.textContent;
      btn.textContent = "✓";
      setTimeout(() => { btn.textContent = old; }, 1200);
    });
  });
  apply();
})();
</script>
"""

out_path.write_text(head + html.replace("__PAYLOAD__", payload) + "\n")
print(f"wrote {out_path} ({count} tools, searchable directory)")
PY

rm -f /tmp/jarvy-tools-index.json

echo ""
echo "Done. Run \`mkdocs build --strict\` to verify."
