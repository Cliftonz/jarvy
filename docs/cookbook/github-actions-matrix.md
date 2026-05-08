---
title: "Recipe: GitHub Actions matrix — Jarvy"
description: "Run jarvy validate, drift check, and setup across macOS, Linux, and Windows in a GitHub Actions matrix to catch cross-platform breakage early."
tags:
  - cookbook
  - ci
  - github-actions
---

# Recipe: GitHub Actions matrix

## Problem

Your `jarvy.toml` works on the laptops you have access to. You want to know — before merge — whether a change works on Linux, Windows, and macOS, and whether the committed `.jarvy/state.json` baseline still matches.

---

## Config

```yaml title=".github/workflows/jarvy.yml"
name: Jarvy
on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

jobs:
  validate:
    name: Validate (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4

      - name: Install Jarvy (Unix)
        if: runner.os != 'Windows'
        run: curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash

      - name: Install Jarvy (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: irm https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.ps1 | iex

      - name: Add Jarvy to PATH (Unix)
        if: runner.os != 'Windows'
        run: echo "$HOME/.jarvy/bin" >> "$GITHUB_PATH"

      - name: Add Jarvy to PATH (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: Add-Content $env:GITHUB_PATH "$env:USERPROFILE\.jarvy\bin"

      - name: Validate config
        run: jarvy validate --strict

      - name: Show plan (dry-run)
        run: jarvy setup --dry-run

      - name: Provision
        env:
          CI: "true"
          JARVY_TEST_MODE: "1"
        run: jarvy setup --ci

      - name: Drift check
        run: jarvy drift check --format json
```

---

## Why it works

| Step | What it catches |
|---|---|
| `validate --strict` | Schema errors, unknown tools, role cycles — fails fast before any installs. |
| `setup --dry-run` | The full plan — useful artifact for PR review. |
| `setup --ci` | Real install. Catches Linux/Windows-specific package issues that only surface on those platforms. |
| `drift check` | Whether `.jarvy/state.json` matches what the install actually produced. Fails the PR if a contributor bumped `jarvy.toml` but forgot to refresh the baseline. |
| `fail-fast: false` | One platform failing doesn't cancel the others — you see all three results. |

Caches and prebuilt actions can speed this up further; here we keep it minimal and explicit.

---

## Variations

**Skip Windows for a Unix-only project:**

```yaml
matrix:
  os: [ubuntu-latest, macos-latest]
```

**Test multiple roles:**

```yaml
matrix:
  os: [ubuntu-latest, macos-latest]
  role: [frontend, backend, devops]
- run: jarvy setup --role ${{ matrix.role }} --ci
```

**Cache between runs:**

```yaml
- uses: actions/cache@v4
  with:
    path: |
      ~/.cargo
      ~/.npm
      ~/Library/Caches/Homebrew
    key: ${{ runner.os }}-jarvy-${{ hashFiles('jarvy.toml', '.jarvy/state.json') }}
```

**Validate without installing (lightweight check):**

For most PRs you don't need to run the actual install — just validate. Fast feedback in seconds:

```yaml
- run: jarvy validate --strict
- run: jarvy diff
```

**Other CI providers** — Jarvy detects 11 CI environments and adjusts behavior automatically. The same shell commands work in GitLab, CircleCI, Buildkite, Jenkins:

```bash
jarvy validate --strict
jarvy setup --ci
jarvy drift check --format json
```

---

## Caveats

- **Windows runners are slow** — typically 2-3× longer than Linux. Skip Windows on every PR if it's not a primary target; run it on `main` only.
- **`jarvy setup --ci` is non-interactive** — it auto-answers prompts. Hooks that ask for input will fail. Make hooks unattended.
- **Cache invalidation is on you** — if `.jarvy/state.json` is committed, the cache key includes it; otherwise CI's first run is always cold.
- **Self-hosted runners need Jarvy preinstalled** or a smaller `jarvy update` step.

---

## See also

- [CI/CD integration guide](../ci-cd.md) — provider-specific details
- [Drift detection in CI](../concepts/drift-baseline.md#drift-in-ci)
- [Onboarding tutorial — CI step](../tutorials/team-onboarding.md)
