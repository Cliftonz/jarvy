---
title: "Logging & Debug Tickets - Jarvy"
description: "File-based logs with rotation and sanitization, plus debug ticket bundles for support."
---

# Logging & Debug Tickets

Jarvy writes structured logs to `~/.jarvy/logs/` and can bundle a full diagnostic snapshot ("ticket") for support requests. Sensitive data is redacted before anything hits disk.

## Log Files

- Location: `~/.jarvy/logs/jarvy.log`
- Rotated: `jarvy.log.1.gz`, `jarvy.log.2.gz`, …
- Format: text or JSON (configurable)
- Levels: `error`, `warn`, `info`, `debug`, `trace`
- Permissions: the directory is `0700` and every `jarvy.log*` file is
  narrowed to `0600` at startup. The sanitizer below is a denylist — it
  redacts the secret shapes it recognizes and writes everything else
  verbatim — so the file mode is the second line of defense, not the
  first.

## Configuration

```toml
# jarvy.toml
[logging]
enabled = true
level = "info"           # error | warn | info | debug | trace
format = "json"          # text | json
max_file_size = "10MB"   # rotate when exceeded
max_files = 5            # keep N rotated files
max_age_days = 30        # delete logs older than this
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `true` | Master switch |
| `level` | enum | `info` | Minimum level captured |
| `format` | enum | `text` | `text` or `json` |
| `max_file_size` | string | `10MB` | Rotation threshold |
| `max_files` | int | `5` | Rotated file count to retain |
| `max_age_days` | int | `30` | Compressed-log retention |

The log file captures `info` and above. `-v`/`-vv`/`-vvv` widen it to
`debug`/`trace` so those events reach a persistent sink instead of only
the terminal — reproduce a problem under `-vv`, then run `jarvy ticket
create`. `--quiet` does **not** narrow it: it quiets the console only, so
a silenced run still produces a usable bundle.

## Sanitizer

The `Sanitizer` runs on every log line before it's written. It redacts:

- API keys (`sk_live_...`, `xoxb-...`, `ghp_...`, generic `*_API_KEY` patterns)
- Bearer tokens and JWTs
- Email addresses (in messages — addresses you log intentionally are still your responsibility)
- Passwords from URLs (`https://user:pass@host`)
- AWS access key IDs (`AKIA...`)

Redacted values are replaced with `[REDACTED:type]`. Custom patterns can be added via the sanitizer config.

## Commands

```bash
jarvy logs view                          # Follow recent logs
jarvy logs view --lines 200              # Last 200 lines
jarvy logs view --level error            # Errors only
jarvy logs stats                         # Counts, file sizes, oldest entry
jarvy logs clean                         # Remove rotated logs older than max_age_days
jarvy logs clean --all                   # Wipe everything
jarvy logs config                        # Show effective logging config
```

## Debug Tickets

When something breaks, a ticket bundles everything a maintainer needs into a single ZIP:

```bash
jarvy ticket create                  # General-purpose bundle
jarvy ticket create --tool docker    # Focused on a single tool
jarvy ticket list                    # Existing tickets in ~/.jarvy/tickets/
jarvy ticket show <id>               # Inspect bundle contents
jarvy ticket clean --older-than 30   # Remove old tickets
```

### What's In a Ticket

| File | Contents |
|------|----------|
| `system.json` | OS, arch, kernel, available package managers, env vars (sanitized) |
| `tools.json` | Each configured tool: detected version, path, install method |
| `config.toml` | Sanitized copy of `jarvy.toml` (secrets removed) |
| `logs/` | Recent log files (rotated + current) |
| `versions.txt` | Output of `--version` for each tool |
| `errors.txt` | Last N error/warn log lines |

Tickets are named `JARVY-YYYYMMDD-xxxxxxxx.zip` and saved to `~/.jarvy/tickets/`.

### What's Excluded

- File paths inside `$HOME` are stripped to `~/...`
- Secrets, tokens, API keys (sanitized)
- Hostname (replaced with `redacted-host`)
- Network proxy credentials

You can hand a ticket to a maintainer or paste its summary into a GitHub issue without leaking secrets.

## Programmatic Use

If you're scripting around Jarvy, prefer the JSON log format:

```toml
[logging]
format = "json"
```

Each line is a single JSON object with `timestamp`, `level`, `target`, `message`, plus structured fields from `tracing` spans.

## Module

- Source: `src/logging/`, `src/ticket/`
- Key types: `LoggingConfig`, `Sanitizer`, `RotatingFileWriter`, `LogRotator`, `TicketCollector`, `TicketBundler`
- Stack: `tracing` + `tracing-appender` + `zip` for bundling
