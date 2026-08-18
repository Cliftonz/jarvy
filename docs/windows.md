---
title: "Windows — Jarvy"
description: "Windows-only setup knobs: .SH PATHEXT + .sh file association so bash scripts don't open in Notepad. Per-user (no admin), opt-in, cross-platform-safe config."
tags:
  - guides
  - windows
---

# Windows

The `[windows]` block fixes a specific paper cut: on Windows, bash scripts (`.sh` files) don't run when double-clicked in Explorer and don't invoke bash when typed at a cmd/PowerShell prompt. They open in Notepad (or "How do you want to open this?") because Windows has no default `.sh` association and doesn't include `.SH` in PATHEXT.

Assuming Git for Windows is installed (`bash.exe` at `C:\Program Files\Git\bin\`), two knobs fix this at the per-user level (HKCU only, no admin needed).

Teams pick either or both. The block parses on every OS but the phase is a no-op on non-Windows, so a cross-platform team can commit one `jarvy.toml`.

---

## Configuration

```toml
[windows]
enabled = true                   # default true

# Option B — add .SH to the user's PATHEXT env var so `myscript.sh`
# invokes bash from cmd/PowerShell without typing `bash myscript.sh`.
sh_pathext = true                # default false

# Option C — set the Windows file association for .sh files.
#   "off"  = do not touch associations (default)
#   "open" = route the "open" verb to bash.exe, leave "edit" alone
#            so VS Code / Notepad++ still edit .sh for editing
sh_association = "open"          # default "off"

# Override the resolved bash.exe path — validated for NUL/quote/newline
# to keep `reg add /d "..."` safe.
bash_path_override = "D:\\tools\\git\\bin\\bash.exe"   # default: auto-detect

# Trust gate — remote configs (jarvy setup --from <url>) refused
# without this. Mirrors [git_hooks] / [dotfiles] / [packages].
allow_remote = false             # default false
```

Both knobs default to `false` — the block's presence alone doesn't change anything. Set one or both explicitly.

---

## Option B — `sh_pathext`

Appends `.SH` to `HKCU\Environment\PATHEXT` (per-user). After a fresh shell is opened, typing `myscript.sh` at cmd/PowerShell will invoke bash. `myscript` (no extension) also works if the extension resolution finds the `.sh` file first.

**Idempotent** — reruns detect `.SH` already present (case-insensitive) and skip the write, emitting `windows.pathext_unchanged`. Safe to leave in place across setup runs.

**Effective when a new shell starts.** The Windows env-var change is picked up by newly-spawned processes; existing terminals need a restart.

---

## Option C — `sh_association`

Sets the `.sh` file association at the per-user level:

- `HKCU\Software\Classes\.sh` → `Jarvy.BashScript`
- `HKCU\Software\Classes\Jarvy.BashScript\shell\open\command` → `"C:\Program Files\Git\bin\bash.exe" "%1" %*`

**Only the `"open"` verb is routed.** The `"edit"` verb is left alone, so Shift+right-click → Edit still opens `.sh` in the user's chosen editor (VS Code, Notepad++, whatever the shell had before).

**Distinct `Jarvy.BashScript` ProgId** so a future `remove` command can idempotently clean up jarvy's writes without touching a user-authored `.sh` class.

Modes:

| Value | Effect |
|---|---|
| `"off"` | Do not touch associations (default) |
| `"open"` | Route `.sh` "open" verb to bash.exe; preserve "edit" |

Future modes could add `"open_and_edit"` or `"wsl"` (route via `wsl.exe -e bash`) — but no plans today.

---

## Bash path auto-detection

Jarvy checks these paths in order and uses the first that exists:

1. `C:\Program Files\Git\bin\bash.exe`
2. `C:\Program Files (x86)\Git\bin\bash.exe` (WOW64 mirror for 32-bit Git installs)

Override with `bash_path_override` for non-default installs (Chocolatey / Scoop / MSI installer using a custom path) or if you prefer `wsl.exe`:

```toml
[windows]
sh_association = "open"
bash_path_override = "C:\\Windows\\System32\\wsl.exe"
# ...though wsl needs an "-e bash" arg — see caveat below
```

**Security:** the override path is passed to `reg add /d "..."` verbatim. Jarvy refuses paths containing NUL bytes, embedded double-quotes, or newlines at config-load time — a hostile remote config can't inject a second registry command through this field.

---

## Non-Windows targets

The block parses on every OS. On non-Windows targets, the phase short-circuits with a `windows.phase_skipped { reason = "not_windows" }` event and does nothing. A single `jarvy.toml` works for a cross-platform team.

---

## Telemetry

- `windows.phase_started` / `windows.phase_completed` — lifecycle with the resolved outcome per knob
- `windows.phase_skipped` — reason bounded: `"disabled"` / `"not_windows"` / `"nothing_to_do"` / `"remote_refused"`
- `windows.pathext_applied` / `windows.pathext_unchanged` — HKCU PATHEXT write result
- `windows.pathext_failed` — `reg add HKCU\Environment\PATHEXT` returned non-zero
- `windows.sh_association_applied` / `windows.sh_association_unchanged` — assoc write result
- `windows.sh_association_failed` — bash.exe not found, or `reg add` refused
- `windows.remote_refused` — remote-origin config declared `[windows]` without `allow_remote = true`

All gated behind the standard telemetry consent gate.

---

## Trust boundary

Same shape as `[git_hooks] allow_remote` — a remote `jarvy.toml` fetched via `jarvy setup --from <url>` is refused for the Windows phase unless `[windows] allow_remote = true` is set **in the source config**. Prevents a friendly-looking remote config from routing a Windows user's `.sh` files to an attacker binary via `bash_path_override`.

---

## Troubleshooting

- **Change doesn't take effect in my current shell** — env var writes only reach new processes. Open a fresh cmd/PowerShell/Terminal window.
- **`.sh` still opens in Notepad after `sh_association = "open"`** — Windows keeps a per-user "User Choice" that overrides class associations. Check Settings → Apps → Default apps → Choose defaults by file type; delete any override for `.sh` there.
- **`sh_association_failed` event with "bash.exe not found"** — Git for Windows isn't installed at a standard path. Install it (via `[provisioner] git = "latest"` or manually) or set `bash_path_override`.
- **"Not on Windows" advisory line during setup** — the phase saw `[windows]` in your config but you're on macOS/Linux. Cosmetic; safe to ignore, or delete the block if this repo is Windows-only.

---

## Related

- [Git hooks](git-hooks.md) — related trust-gated setup phase with the same `allow_remote` shape
- [Configuration reference](configuration.md) — full `[windows]` schema
