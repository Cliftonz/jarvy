# jarvy-mcp

npm wrapper for [Jarvy](https://github.com/Cliftonz/jarvy)'s Model Context
Protocol (MCP) server. On install it downloads the platform-native `jarvy`
release binary from GitHub Releases, verifies it against the release
`SHA256SUMS.txt`, and exposes two bins:

- **`jarvy-mcp`** — runs `jarvy mcp` (the MCP server, stdio transport).
  This is what MCP clients should invoke.
- **`jarvy`** — full jarvy CLI passthrough.

## Usage with MCP clients

```json
{
  "mcpServers": {
    "jarvy": {
      "command": "npx",
      "args": ["-y", "jarvy-mcp"]
    }
  }
}
```

Or install globally:

```bash
npm install -g jarvy-mcp
jarvy-mcp          # starts the MCP server on stdio
jarvy --version    # full CLI is available too
```

## Supported platforms

| Platform | Arch | Release triple |
|---|---|---|
| macOS | arm64 (Apple Silicon) | `aarch64-apple-darwin` |
| Linux | x86_64 | `x86_64-unknown-linux-musl` (static — glibc and musl) |
| Linux | arm64 | `aarch64-unknown-linux-gnu` |
| Linux | armv7 | `armv7-unknown-linux-gnueabihf` |
| Windows | x86_64 | `x86_64-pc-windows-msvc` |

Intel macOS has no prebuilt binary — use `cargo install jarvy` or
`brew install Cliftonz/tap/jarvy` instead.

## Environment variables

| Variable | Effect |
|---|---|
| `JARVY_NPM_SKIP_DOWNLOAD=1` | Skip the postinstall download; bins fall back to a `jarvy` already on PATH |
| `JARVY_SKIP_CHECKSUM=1` | Skip SHA256 verification (NOT recommended) |
| `JARVY_VERSION` | Download a specific jarvy release instead of the package's pinned version |

## Versioning

The package version is kept in lockstep with jarvy releases: `jarvy-mcp@X.Y.Z`
downloads `jarvy vX.Y.Z`.

## Security

The postinstall verifies the downloaded archive's SHA256 against the release's
`SHA256SUMS.txt` before extracting, mirroring `dist/scripts/install.sh`. A
mismatch aborts the install. Release artifacts are additionally signed with
Sigstore — see the [release notes](https://github.com/Cliftonz/jarvy/releases)
for `cosign verify-blob` instructions.

## Development

```bash
npm test                                   # unit tests (node --test)
JARVY_NPM_SKIP_DOWNLOAD=1 npm install      # install without downloading
```

## License

MIT OR Apache-2.0, same as jarvy itself.
