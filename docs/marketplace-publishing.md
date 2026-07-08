# Marketplace Publishing — Maintainer Setup

Jarvy ships four distributable integrations from this monorepo, each on its
own release cadence. The code, build automation, and release workflows are all
in place; this document is the **manual, one-time setup** a maintainer must do
(create accounts, mint tokens, add repo secrets) plus the **per-release steps**.

Nothing here can be automated — it all requires accounts and credentials only
the project owner can create.

## TL;DR — what needs a human

| Integration | One-time account/secret | First publish is manual? |
|---|---|---|
| GitHub Action | none (uses `GITHUB_TOKEN`) | ✅ tick "Publish to Marketplace" once per major |
| VS Code extension | `VSCE_PAT` secret (+ `OVSX_PAT` for Cursor/forks) | publisher must exist first |
| JetBrains plugin | `PUBLISH_TOKEN` + signing trio secrets | Marketplace approves first upload |
| npm / Docker / MCP registry (PRD-021) | npm + ghcr + MCP registry | see `dist/mcp-registry/README.md` |

---

## 1. GitHub Action

The action lives at the repo root (`action.yml`) and versions on its **own**
tag line (`action-vX.Y.Z` + moving `action-vX`), independent of the CLI's
`vX.Y.Z` tags. See `docs/github-action.md` for the full scheme.

**One-time setup:** none — the release workflow uses the built-in
`GITHUB_TOKEN`.

**Per release:**
1. `git tag action-v1.0.0 && git push origin action-v1.0.0`
2. `.github/workflows/action-release.yml` fires: it force-moves `action-v1`
   to that commit and creates a GitHub Release.
3. **First release of each major line only:** open that Release in the GitHub
   UI and tick **"Publish this Action to the GitHub Marketplace."** There is no
   API for this checkbox — it must be done by hand once per major (`action-v1`,
   later `action-v2`). Subsequent `action-v1.*` pushes keep the listing current
   automatically.

Consumers then use `uses: Cliftonz/jarvy@action-v1`.

---

## 2. VS Code extension (and Cursor / Windsurf / VSCodium)

Versions off `editors/vscode/package.json`. Published by
`.github/workflows/vscode-publish.yml`, triggered by a `vscode-vX.Y.Z` tag.

**One-time setup:**
1. **Create a publisher** on the Visual Studio Marketplace
   (<https://marketplace.visualstudio.com/manage>), backed by an Azure DevOps
   organization. Set `"publisher"` in `editors/vscode/package.json` to its ID
   (currently the placeholder `jarvy`).
2. **Mint a VS Marketplace PAT** (Azure DevOps → User Settings → Personal
   Access Tokens → scope: *Marketplace → Manage*). Add it as the repo secret
   **`VSCE_PAT`**.
3. **(Cursor/forks) Create an Open VSX account** at <https://open-vsx.org>,
   publish/claim a namespace, and mint an access token. Add it as the repo
   secret **`OVSX_PAT`**. See §5 for why this covers Cursor.

**Per release:**
1. Bump `version` in `editors/vscode/package.json`; note it in the extension
   `CHANGELOG.md`.
2. `git tag vscode-v0.1.0 && git push origin vscode-v0.1.0`.
3. The workflow packages the `.vsix` and publishes to both registries. Each
   registry step is skipped (with a notice, not a failure) if its token is
   absent — so you can enable VS Marketplace and Open VSX independently.

`workflow_dispatch` offers a package-only dry run that just uploads the `.vsix`
as a build artifact.

---

## 3. JetBrains plugin

Versions off `editors/jetbrains/gradle.properties` (`pluginVersion`). Published
by `.github/workflows/jetbrains-publish.yml`, triggered by a `jetbrains-vX.Y.Z`
tag.

**One-time setup:**
1. **Create a JetBrains Marketplace account** (<https://plugins.jetbrains.com>)
   and a **permanent token** (Profile → My Tokens). Add it as the repo secret
   **`PUBLISH_TOKEN`**.
2. **Generate a signing certificate** (see JetBrains' "Plugin Signing" guide —
   an RSA keypair + self-signed chain). Add three repo secrets:
   **`CERTIFICATE_CHAIN`** (PEM chain), **`PRIVATE_KEY`** (PEM key),
   **`PRIVATE_KEY_PASSWORD`**.
3. The **first upload of a new plugin** goes into a moderation queue on the
   Marketplace — JetBrains reviews it before it's publicly listed. Later
   versions publish immediately.

**Per release:**
1. Bump `pluginVersion` in `editors/jetbrains/gradle.properties`.
2. `git tag jetbrains-v0.1.0 && git push origin jetbrains-v0.1.0`.
3. The workflow runs `buildPlugin` + `verifyPlugin`, then `signPlugin` +
   `publishPlugin`. If `PUBLISH_TOKEN` is absent it skips publishing (with a
   notice); if the signing trio is absent it publishes unsigned.

A `pluginVersion` like `0.2.0-beta.1` auto-routes to the matching Marketplace
pre-release channel.

---

## 4. npm / Docker / MCP registry (PRD-021)

The MCP-server distribution artifacts (npm wrapper, Docker image, MCP registry
`server.json`) have their own maintainer runbook — see
`dist/mcp-registry/README.md` and `docs/mcp-server.md`. In brief: publish the
npm package (`NPM_TOKEN`), push the ghcr image (the `docker-publish` workflow
on a tag), then submit `server.json` to the MCP registry (which requires the
npm + ghcr artifacts to exist first, for ownership proof).

---

## 5. Does the VS Code extension work in Cursor?

**Yes — with no separate build.** Cursor is a VS Code fork and uses the same
extension API, so the extension's behavior (jarvy.toml diagnostics, the
setup/doctor/validate commands, status bar) works identically.

The only difference is *distribution*. In June 2025 Cursor moved its in-app
extension marketplace to the **Open VSX Registry**, because Microsoft's terms
restrict the VS Marketplace to Microsoft's own products. So:

- **VS Code** installs from the VS Marketplace (`VSCE_PAT` path).
- **Cursor, Windsurf, VSCodium** install from **Open VSX** (`OVSX_PAT` path).

Because `vscode-publish.yml` already publishes the same `.vsix` to **both**
registries, one release covers VS Code *and* Cursor/forks — no Cursor-specific
work. Users can also `Extensions: Install from VSIX…` with the `.vsix` build
artifact directly. (Microsoft's proprietary extensions are the only things that
don't cross-publish; the jarvy extension uses only standard APIs, so it's
fully portable.)

---

## Setup checklist

- [ ] VS Code: create Marketplace publisher, set `package.json` `publisher`, add `VSCE_PAT`
- [ ] Cursor/forks: create Open VSX namespace, add `OVSX_PAT`
- [ ] JetBrains: create Marketplace account, add `PUBLISH_TOKEN`
- [ ] JetBrains: generate signing cert, add `CERTIFICATE_CHAIN` / `PRIVATE_KEY` / `PRIVATE_KEY_PASSWORD`
- [ ] GitHub Action: cut `action-v1.0.0`, tick the Marketplace box on the Release once
- [ ] npm/Docker/MCP: follow `dist/mcp-registry/README.md`
