# MCP Registry Submission (PREP)

This directory holds `server.json` — Jarvy's entry for the
[official MCP registry](https://registry.modelcontextprotocol.io/)
(`io.github.cliftonz/jarvy`). It validates against the
`2025-12-11` schema at
`https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json`.

**Nothing here submits anything.** Publishing to the registry is a
maintainer-only action with hard prerequisites — see below.

## Prerequisites (must exist BEFORE publishing)

The registry verifies that every listed package is real and owned by the
publisher at submit time:

1. **npm package** — `jarvy-mcp@<version>` must be live on
   registry.npmjs.org. Ownership proof: the `mcpName` field in
   `packages/npm/package.json` must equal `io.github.cliftonz/jarvy`
   (already set).
2. **OCI image** — `ghcr.io/cliftonz/jarvy:<version>` must be pushed and
   publicly pullable. Ownership proof: the
   `io.modelcontextprotocol.server.name` label baked into
   `dist/docker/Dockerfile` must equal `io.github.cliftonz/jarvy`
   (already set). The package must be set to **public** visibility in the
   GitHub Packages settings.
3. **Versions agree** — `version` in `server.json` and both package
   `identifier`/`version` fields must reference the same released jarvy
   version.

## Submission steps (maintainer)

```bash
# 1. Install the publisher CLI
brew install mcp-publisher
# or download a release binary:
#   https://github.com/modelcontextprotocol/registry/releases

# 2. Authenticate. io.github.* namespaces authenticate via GitHub OAuth —
#    log in as the Cliftonz account (or a member with repo admin).
cd dist/mcp-registry
mcp-publisher login github

# 3. Validate + publish
mcp-publisher publish
```

The registry is append-only per version: publishing `0.5.2` once is
permanent; ship a new `version` to update. After the first publish, keep
`server.json` versions in lockstep with releases (candidate for a
publish-packages.yml job later — deliberately NOT wired up yet).

## Verification after publish

```bash
curl -s "https://registry.modelcontextprotocol.io/v0/servers?search=io.github.cliftonz/jarvy" | jq .
```

## References

- server.json format: <https://github.com/modelcontextprotocol/registry/blob/main/docs/reference/server-json/generic-server-json.md>
- Official registry requirements: <https://github.com/modelcontextprotocol/registry/blob/main/docs/reference/server-json/official-registry-requirements.md>
- Registry repo: <https://github.com/modelcontextprotocol/registry>
