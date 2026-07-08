# Jarvy for VS Code

Validate and manage [`jarvy.toml`](https://github.com/Cliftonz/jarvy) dev-environment
configuration files directly from VS Code.

Jarvy is a cross-platform CLI that provisions developer environments from a
`jarvy.toml` manifest using native package managers (brew, apt/dnf, winget, …).
This extension surfaces `jarvy validate` diagnostics in the Problems panel and
gives you one-click access to `jarvy setup` and `jarvy doctor`.

## Features

- **Live validation.** Runs `jarvy validate --strict --file <path> --format json`
  on open, on save, and (debounced) as you type. Errors and warnings appear in
  the Problems panel, mapped to the reported line when jarvy provides one.
- **Status bar indicator.** Shows whether the active `jarvy.toml` is valid,
  invalid (with error count), or missing.
- **Commands** (Command Palette → `Jarvy:`):
  - `Jarvy: Validate Configuration` — re-run diagnostics on demand.
  - `Jarvy: Run Setup` — run `jarvy setup --file <path>` in an integrated terminal.
  - `Jarvy: Run Doctor` — run `jarvy doctor --format json` and show the output.
- **Quick Fix.** On an "Unknown tool" diagnostic, offers *Run jarvy setup*.
- **Language support.** Associates `jarvy.toml` with a lightweight `jarvy-toml`
  language for TOML-style syntax highlighting.

## Requirements

You need the `jarvy` CLI on your `PATH`. Install it with:

```sh
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash
```

See the [installation docs](https://github.com/Cliftonz/jarvy#installation) for
other methods (Homebrew, Cargo, prebuilt binaries).

If jarvy is installed somewhere off your `PATH`, set `jarvy.executablePath` in
your settings. When the binary cannot be found, the extension shows a warning
with a link to the docs rather than failing silently.

## Extension Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `jarvy.executablePath` | `jarvy` | Path to the jarvy executable. |
| `jarvy.validate.onSave` | `true` | Re-validate on save. |
| `jarvy.validate.onChange` | `true` | Re-validate (debounced) while typing. |
| `jarvy.validate.strict` | `true` | Pass `--strict` (warnings become errors). |
| `jarvy.validate.debounceMs` | `500` | On-change debounce interval (ms). |

## Development

```sh
cd editors/vscode
npm install
npm run compile      # tsc -> out/
npm run watch        # incremental compile
npm run check        # tsc --noEmit (type-check only)
```

Press `F5` in VS Code to launch an Extension Development Host.

## Versioning & releasing (maintainers)

The extension version lives in `package.json` (`version`) and is **independent
of the jarvy CLI's git tags** — the marketplace reads the version from the
uploaded `.vsix`, not from a git tag.

To release:

1. Bump `version` in `editors/vscode/package.json` and note it in `CHANGELOG.md`.
2. Tag `vscode-vX.Y.Z` and push it (the `vscode-` prefix keeps CLI `vX.Y.Z`
   releases from triggering the publish workflow).

That push triggers `.github/workflows/vscode-publish.yml`, which type-checks,
packages the `.vsix`, and publishes to the **VS Marketplace** (`VSCE_PAT`
secret) and **Open VSX** (`OVSX_PAT` secret). Either publish step is skipped
with a notice if its token isn't configured, and `workflow_dispatch` offers a
package-only dry run. `publisher` in `package.json` is a placeholder until a
marketplace publisher account exists.

## License

MIT — see the root of the [jarvy repository](https://github.com/Cliftonz/jarvy).
