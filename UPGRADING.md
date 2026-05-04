# Upgrading Jarvy

This document covers breaking changes and migration steps between versions.

## Unreleased (development)

### Lockfile checksum format changed

The lockfile (`jarvy.lock`) checksum algorithm was upgraded from a non-cryptographic hash (`DefaultHasher`) to SHA-256 for integrity verification.

**Impact:** Existing lockfiles will show checksum mismatches after upgrading.

**Migration:** Regenerate your lockfile:

```bash
jarvy lock generate
```

### `--insecure` flag removed

The `--insecure` flag on `jarvy setup --from <url>` was removed. It was never implemented (TLS was always verified). If you had scripts using this flag, remove it.

**Before:**
```bash
jarvy setup --from https://example.com/config.toml --insecure
```

**After:**
```bash
jarvy setup --from https://example.com/config.toml
```

### Config `[commands]` section added

A new optional `[commands]` section lets you configure the interactive menu commands:

```toml
[commands]
run = "npm start"
test = "npm test"
setup = "make dev-setup"
```

Custom commands display a security confirmation prompt before execution. Default commands (`cargo run`, `cargo test`) run without prompting.

### Telemetry `disable` now clears machine fingerprint

Running `jarvy telemetry disable` now also clears the machine fingerprint from `~/.jarvy/config.toml`. Previously, the fingerprint persisted even after disabling telemetry.

### MCP auto-approve preference

When a user selects "Always" during MCP tool install confirmation, the preference is now persisted to `~/.jarvy/config.toml` under `[mcp]`:

```toml
[mcp]
auto_approve_installs = true
```

To reset, set it to `false` or remove the section.

## General Upgrade Steps

1. Update Jarvy:
   ```bash
   jarvy update
   # or
   cargo install jarvy
   ```

2. Regenerate lockfile (if using `jarvy lock`):
   ```bash
   jarvy lock generate
   ```

3. Review `jarvy telemetry status` to confirm your preferences.
