# Privacy Policy

Jarvy collects optional, anonymized telemetry to help improve the tool. Telemetry is **opt-in** by default and can be disabled at any time.

## What is collected

When telemetry is enabled, the following data is sent to the configured OTLP endpoint:

### Events

| Event | Data | Purpose |
|-------|------|---------|
| `tool.installed` | Tool name, package manager used, OS, duration | Track which tools are popular and install reliability |
| `tool.failed` | Tool name, OS, error type | Identify broken install paths |
| `setup.completed` | Tool count, success/fail counts, duration | Measure setup reliability |
| `hook.completed` | Hook name, hook type, duration, exit code | Monitor hook health |
| `command.executed` | Command name, duration, success | Understand feature usage |

### Metrics

| Metric | Type | Purpose |
|--------|------|---------|
| `jarvy.tool.requests` | Counter | Total tool install requests |
| `jarvy.tool.installs` | Counter | Installs by status (success/fail) |
| `jarvy.install.duration` | Histogram | Installation time distribution |
| `jarvy.setup.duration` | Histogram | Setup time distribution |

### Machine identifier

A **machine fingerprint** is included with setup events to enable unique device counting. This identifier is:

- A **one-way SHA-256 hash** of hardware attributes (system UUID, CPU core count, OS name, disk serial)
- **Not reversible** — the original hardware details cannot be recovered from the hash
- **Cleared** when you run `jarvy telemetry disable`
- **Visible** via `jarvy telemetry status`

## What is NOT collected

- File paths, directory names, or project names
- Environment variable values or secrets
- User identity, IP address, or location (by Jarvy — your OTLP backend may log source IPs)
- Config file contents
- Git repository URLs or commit hashes
- Hook script contents

All sensitive data (API keys, tokens, passwords, emails, SSH keys) is redacted before any log output via the built-in sanitizer.

## How to control telemetry

```bash
# Check current status and see your machine fingerprint
jarvy telemetry status

# Disable telemetry and clear machine fingerprint
jarvy telemetry disable

# Re-enable telemetry
jarvy telemetry enable

# Preview what events would be sent (without sending)
jarvy telemetry preview
```

### Environment variables

| Variable | Effect |
|----------|--------|
| `JARVY_TELEMETRY=0` | Disable telemetry for this session |
| `JARVY_TELEMETRY=1` | Enable telemetry for this session |
| `CI=true` | Auto-disables telemetry in CI (unless `JARVY_TELEMETRY=1`) |

### Config file

Edit `~/.jarvy/config.toml`:

```toml
[settings]
telemetry = false    # Disable telemetry

[telemetry]
enabled = false      # Also disables telemetry
```

## Data destination

Telemetry is sent to the configured OTLP endpoint (default: `http://localhost:4318`). You control where this data goes. Jarvy does not operate a centralized telemetry service.

## Questions

If you have privacy concerns, please open an issue at https://github.com/bearbinary/jarvy/issues.
