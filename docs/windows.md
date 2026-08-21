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

## `[windows.wsl]` — WSL2 bridge

For POSIX-only tools (`tmux`, `htop`, `zsh`, `yadm`, `goaccess`, `tilt`) that have no first-party winget/choco package on Windows, the `[windows.wsl]` sub-block bootstraps a named WSL2 distro and (optionally) delegates the whole `jarvy setup` into it. The Linux-side jarvy is the install backend for those tools.

```toml
[windows.wsl]
enabled = false                 # opt-in gate; the tool is inert until true
distro = "Ubuntu"               # v1 supports "Ubuntu" and "Debian"
# name = "jarvy-ubuntu"         # defaults to `distro` verbatim; set for isolation
auto_bootstrap = false          # if WSL / distro missing, prints exact `wsl --install` command
install_location = "auto"       # "auto" -> %LOCALAPPDATA%\jarvy\wsl\<name>
install_jarvy = true            # curl-pipe jarvy inside the distro (skipped if already installed)
jarvy_channel = "stable"        # stable / beta / nightly
run_setup = false               # true = delegate the outer `jarvy setup` into the distro
```

### Flow

1. `jarvy setup` reads `[windows.wsl]`. If `enabled = false`, nothing happens.
2. Probes `wsl --version` (Store-WSL check) and `wsl -l -q` (installed distros).
3. If Store-WSL is missing OR the named distro is missing:
   - `auto_bootstrap = false` → refuses with the exact `wsl --install -d <distro> --name <name>` command for the user to run in an elevated PowerShell. **Jarvy never triggers UAC.**
   - `auto_bootstrap = true` AND the process is elevated → runs the install (prefers `wsl --install --name`; falls back to `wsl --import` with a cached rootfs when the flag is unsupported).
4. If `install_jarvy = true` and jarvy is not already on PATH inside the distro, jarvy runs `wsl -d <name> -u root -- bash -lc "curl -fsSL <install-url> | sh"`.
5. If `run_setup = true` and the outer invocation is `jarvy setup`, the outer setup re-execs `wsl -d <name> -- bash -lc "cd '/mnt/c/...' && jarvy setup"` on the translated project path. The inner jarvy loads the same `jarvy.toml`, installs the actual project tools via apt/dnf, and returns its exit code.

### Trust

- Inherits `[windows] allow_remote`. A remote `jarvy setup --from <url>` where the remote config sets `[windows.wsl] enabled = true` is refused unless the local `[windows] allow_remote = true` is explicit. WSL install is system-wide and persistent; a remote config alone must not trigger it.
- Distro instance name refuses `docker-desktop` / `docker-desktop-data` at config load — those are Docker Desktop's WSL distros and jarvy will not overwrite them.
- Install location must be absolute, non-UNC, no NUL / quotes / newlines. Rejected at config load.

### Legacy WSL

Jarvy refuses to touch DISM. If `wsl --version` returns nothing (pre-Store WSL: Windows 10 <21H2), the tool refuses with the manual `wsl --install --no-launch` command for the user to run in an elevated PowerShell + reboot. Once Store-WSL is present, re-run `jarvy setup`.

### Uninstall

`wsl --unregister <name>` wipes the distro's rootfs including any user files. Jarvy refuses to run this automatically. Running `jarvy tools --remove wsl` emits the `wsl.remove_refused` event and prints the exact `wsl --unregister <name>` command to stderr for you to run manually (exit code 1). The effective name comes from `[windows.wsl] name` if set, else from `distro`.

```
$ jarvy tools --remove wsl
[wsl] Removing WSL is destructive; jarvy will not `wsl --unregister` your distro.
Run manually if you're sure (this wipes the distro's rootfs including any user data):
    wsl --unregister Ubuntu
```

### Telemetry

- `wsl.probe_completed` — `store_based`, `distros_installed`, `target_distro_present`, `jarvy_in_distro`. Instance name NOT emitted (unbounded cardinality).
- `wsl.bootstrap_refused` — bounded `reason` label (`not_opted_in` / `not_store_wsl` / `no_wsl_command` / `auto_bootstrap_off` / `not_elevated` / `reserved_name`).
- `wsl.bootstrap_started` / `wsl.bootstrap_completed` / `wsl.bootstrap_failed` — `base_distro` (bounded: `Ubuntu` / `Debian`), `method` (`wsl_install` / `wsl_import`), `duration_ms`.
- `wsl.jarvy_install_started` / `wsl.jarvy_install_completed` / `wsl.jarvy_install_failed` — `base_distro`, `channel`, `duration_ms`.
- `wsl.setup_delegated` — `base_distro`, `exit_code`, `duration_ms`. Fires from the delegation short-circuit.
- `wsl.path_refused` — `reason = "unc" | "invalid_chars" | "remote_refused"`. Path itself NOT emitted (may carry the account name).

All gated behind the standard telemetry consent gate.

---

## Related

- [Git hooks](git-hooks.md) — related trust-gated setup phase with the same `allow_remote` shape
- [Configuration reference](configuration.md) — full `[windows]` schema
