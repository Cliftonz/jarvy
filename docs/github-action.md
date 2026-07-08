# Jarvy GitHub Action

Install the [Jarvy](https://github.com/Cliftonz/jarvy) CLI in a GitHub Actions
workflow and optionally validate, set up, or doctor your dev environment from a
`jarvy.toml` — in one step, on Linux, macOS, and Windows.

The action lives at the **root of the Jarvy repo** (`action.yml`), so it is
referenced as `Cliftonz/jarvy@v1` (or any tag/branch/SHA).

> Note: there is a separate, repo-internal composite action at
> `.github/actions/setup-jarvy` used by Jarvy's own CI to build from source.
> For your workflows, use the published top-level action documented here.

## Quick start

```yaml
name: Provision check
on: [push, pull_request]

permissions:
  contents: read

jobs:
  jarvy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Cliftonz/jarvy@v1
        # Defaults: install latest stable via the checksum-verified installer,
        # then `jarvy validate --strict ./jarvy.toml`.
```

## Inputs

| Input            | Default        | Description |
|------------------|----------------|-------------|
| `version`        | `latest`       | Release tag to install (`1.4.0`, `v1.4.0`), or `latest` to resolve the newest release on `channel`. **Pin a concrete version for reproducible, cache-friendly CI.** |
| `channel`        | `stable`       | Channel that `latest` resolves from: `stable`, `beta`, or `nightly`. |
| `install-method` | `install-sh`   | `install-sh` downloads a checksum-verified prebuilt binary from GitHub Releases; `cargo` builds from crates.io (`cargo install jarvy --locked`). |
| `run`            | `validate`     | Command to run after install: `none`, `validate` (`jarvy validate --strict`), `setup` (`jarvy setup --ci`), or `doctor`. |
| `config-path`    | `./jarvy.toml` | Path to the `jarvy.toml` the chosen command acts on. |
| `args`           | `''`           | Extra args appended verbatim to the chosen command (e.g. `--format json`). Ignored when `run: none`. |
| `cache`          | `true`         | Cache the installed binary keyed on version/channel/method + runner OS. Set `false` to always reinstall. |

## Outputs

| Output          | Description |
|-----------------|-------------|
| `jarvy-version` | Installed version, as reported by `jarvy --version`. |
| `jarvy-path`    | Absolute path to the installed `jarvy` binary. |

## How installation works

- **`install-method: install-sh`** runs the canonical installer
  (`dist/scripts/install.sh` on Linux/macOS, `dist/scripts/install.ps1` on
  Windows). Both **verify the download's SHA-256** against the release's
  `SHA256SUMS.txt` before extracting anything — a mismatch aborts the run.
  The binary is installed to `~/.jarvy/bin` and added to `GITHUB_PATH`.
- **`install-method: cargo`** runs `cargo install jarvy --locked` (optionally
  pinned with `--version`). Requires a Rust toolchain (`cargo`) on `PATH`,
  which GitHub-hosted runners provide by default.

### Caching

`Cargo.lock` is intentionally gitignored in this repo, so there is no lockfile
hash to key a cache on. Instead the cache key is the resolved install identity
plus the runner OS:

```
jarvy-<runner.os>-<install-method>-<channel>-<version>
```

When you pin `version` to a concrete tag, cache hits are exact and fast. With
`version: latest`, one cache entry is shared per channel and may serve a
slightly older binary until you bump the pin — pin a version (or set
`cache: false`) if you need every run to fetch the absolute newest build.

## Problem matcher

For `run: validate`, the action registers a
[problem matcher](https://github.com/actions/toolkit/blob/main/docs/problem-matchers.md)
(`.github/problem-matchers/jarvy.json`). It parses `jarvy validate`'s
`[ERROR]` / `[WARN]` lines (including the optional `Line N:` prefix) and
surfaces them as annotations in the workflow's **Problems / Annotations** UI.

Because the action validates with `--strict`, warnings are build-failing, so
both `[ERROR]` and `[WARN]` lines are surfaced as error annotations —
consistent with the strict exit code. The matcher tolerates ANSI color codes,
so it works whether or not the runner strips them.

Jarvy prints the config path in a header line rather than on each diagnostic,
so annotations are attached to the workflow run rather than to a specific
file/line pair.

## Full example: matrix + outputs + JSON

```yaml
name: Jarvy CI
on: [push, pull_request]

permissions:
  contents: read

jobs:
  provision:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      # 1) Install a pinned version and validate the repo config.
      #    The problem matcher annotates any validation failures.
      - id: jarvy
        uses: Cliftonz/jarvy@v1
        with:
          version: 1.4.0
          run: validate
          config-path: ./jarvy.toml

      - name: Show what was installed
        shell: bash
        run: |
          echo "Installed jarvy ${{ steps.jarvy.outputs.jarvy-version }}"
          echo "Binary at ${{ steps.jarvy.outputs.jarvy-path }}"

      # 2) Machine-readable validation for later steps / gating.
      - name: Validate (JSON)
        shell: bash
        run: jarvy validate --strict --file ./jarvy.toml --format json

  setup:
    needs: provision
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # Install-only, then run setup yourself with your own flags.
      - uses: Cliftonz/jarvy@v1
        with:
          version: 1.4.0
          run: none
      - run: jarvy setup --file ./jarvy.toml --ci --dry-run
```

## Notes

- The action is a **composite** action; every step runs on the workflow
  runner. No Docker image is pulled.
- Telemetry is disabled (`JARVY_TELEMETRY=0`) for all steps the action runs.
- `run: setup` passes `--ci` for non-interactive execution. Add
  `args: --dry-run` to preview without mutating the runner.
