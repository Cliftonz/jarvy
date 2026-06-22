# Remote Tool Registry

Jarvy can subscribe to a curated tool-definition registry hosted on HTTPS so new tools become available across a fleet without shipping a new CLI release. Configure in `~/.jarvy/config.toml`, run `jarvy registry sync`, and the next `jarvy setup` picks up the new tools.

## When to use this

The built-in tool registry (the 240+ tools in `src/tools/`) is compiled into the CLI binary. Adding a new tool to the built-in registry requires a Jarvy release. The remote registry decouples that — your platform team can publish a new tool TOML to a Git repo, sign it, and dev machines pull it down on the next sync.

If you need a private one-off tool only your machine needs, use the user-plugin path instead: drop a TOML at `~/.jarvy/tools.d/<name>.toml`. See `src/tools/plugins.rs` module doc. The remote registry is the multi-user / org-wide version of the same mechanism.

## Trust model

The registry config lives **only** in `~/.jarvy/config.toml` (global, user-owned). Project-level `jarvy.toml` files cannot subscribe to a registry — the `[registry]` section is not parsed from project config. This narrowing prevents a hostile project config from pointing the runtime at an attacker registry.

Every registry config pins:

- An HTTPS URL (refused if not `https://`)
- A Sigstore identity-regexp the manifest signature must satisfy
- An OIDC issuer URL for the signing cert

The defaults point at the canonical `bearbinary/jarvy-tools` repo's release workflow. Self-hosted registries override both fields.

Per-tool TOMLs are sha256-pinned in the manifest. A swap-out attack on an individual tool URL is caught by the per-tool sha check; manifest tampering is caught by the cosign signature.

`require_signature = false` exists as an escape hatch for local development. Jarvy emits a stderr warning every sync when it's set, and the cached `meta.json` records `"signature_verified": false` so a later audit can spot it.

## Configuration

```toml
# ~/.jarvy/config.toml

[registry]
url = "https://raw.githubusercontent.com/bearbinary/jarvy-tools/main/registry/"
enabled = true

# Optional — defaults to the bearbinary/jarvy-tools repo's release workflow.
# Override only if you self-host.
signature_identity_regexp = "^https://github\\.com/bearbinary/jarvy-tools/\\.github/workflows/.*\\.yml@refs/heads/main$"
signature_oidc_issuer = "https://token.actions.githubusercontent.com"

# Default true. Set false ONLY for local mirror testing.
require_signature = true
```

`enabled` defaults to `false` so a stray `[registry] url = ...` line in a config-management template doesn't silently subscribe a fleet to a third-party feed.

## Commands

```bash
# Fetch + verify + cache. Subsequent jarvy setup/validate picks up
# the synced tools via the plugin loader.
jarvy registry sync

# Show last sync metadata (URL, count, timestamp, signature flag).
jarvy registry status

# Wipe local cache. Synced tools disappear on next startup until you
# run `registry sync` again.
jarvy registry clear
```

## Manifest format

The registry root hosts:

- `manifest.json`
- `manifest.json.sig` (cosign signature, hex-armored)
- `manifest.json.pem` (cosign cert)
- `tools/*.toml` (one per tool, paths relative to root)

`manifest.json` shape:

```json
{
  "schema_version": 1,
  "generated_at": "2026-06-22T20:00:00Z",
  "tools": [
    {
      "name": "tailscale-extra",
      "path": "tools/tailscale-extra.toml",
      "sha256": "1bbc5baa8ab664a83153424eb4831786e86628bfc024c4f5a675f45a534678ef"
    }
  ]
}
```

`schema_version > 1` is refused — Jarvy can't safely interpret a newer schema. Users must upgrade the CLI before they can use a manifest at a higher version.

Each tool TOML follows the existing plugin format (see `src/tools/plugins.rs` module doc):

```toml
name = "tailscale-extra"
command = "tailscale-extra"

[macos]
brew = "tailscale-extra"

[linux]
uniform = "tailscale-extra"

[windows]
winget = "Publisher.TailscaleExtra"
```

## Cache layout

```text
~/.jarvy/tools.d/
└── .remote/
    ├── manifest.json
    ├── manifest.json.sig
    ├── manifest.json.pem
    ├── meta.json             ← {"last_synced_at_unix": ..., "registry_url": ..., "signature_verified": true}
    └── tools/
        ├── foo.toml
        └── bar.toml
```

Cache root is 0700, files are 0600 on Unix. The cache lives inside `tools.d/` so the same plugin loader walks both user-authored and remote-synced TOMLs with identical security gates.

## Publishing your own registry

1. New GitHub repo `<your-org>/jarvy-tools`.
2. Layout: `registry/manifest.json` + `registry/tools/*.toml`.
3. CI workflow that, on every push to `main`:
   - Regenerates `manifest.json` (compute sha256 for each TOML)
   - Signs `manifest.json` with cosign keyless OIDC
   - Commits the signed bundle back, or uploads to a Pages-served path
4. Users add to their `~/.jarvy/config.toml`:
   ```toml
   [registry]
   url = "https://raw.githubusercontent.com/<your-org>/jarvy-tools/main/registry/"
   enabled = true
   signature_identity_regexp = "^https://github\\.com/<your-org>/jarvy-tools/\\.github/workflows/sign\\.yml@refs/heads/main$"
   ```
5. Run `jarvy registry sync`.

## Limitations

- **No auto-sync.** Users run `jarvy registry sync` manually. Auto-sync on `jarvy setup` is a documented follow-up (see CHANGELOG).
- **Single registry per machine.** Multiple registries would need de-duplication semantics on tool-name conflict; deferred.
- **No HTTP caching headers.** Each sync refetches the manifest. Fine for daily-ish cadence; not for sub-minute polling.
- **Remote-config trust narrowing not enforced for project `jarvy.toml`.** Since `[registry]` only exists in global config, this is implicit, but the test that proves it is a follow-up.

## Operator runbook

```bash
# Subscribe to the canonical registry
cat >> ~/.jarvy/config.toml <<'EOF'
[registry]
url = "https://raw.githubusercontent.com/bearbinary/jarvy-tools/main/registry/"
enabled = true
EOF

jarvy registry sync
jarvy tools | grep <new-tool-name>   # should appear
```

If sync fails:

- `registry not configured` → enable via `enabled = true`
- `signature verification failed` → either the identity-regexp doesn't match the actual signer (check the cosign cert via `cosign verify-blob ... --certificate=...`), or the cosign binary isn't on PATH
- `sha256 mismatch` → the manifest entry is out of date relative to the file content. Re-sign the manifest upstream.
- `non-https url refused` → fix the `url` field to use `https://`
