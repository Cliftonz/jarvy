---
title: "CI/CD Integration - Jarvy"
description: "Use Jarvy in CI/CD pipelines to ensure consistent tool versions across local development and CI environments."
---

# CI/CD Integration

Jarvy works in CI pipelines the same way it works locally. Run `jarvy setup` in your CI job and every build gets the same tools your developers use.

## GitHub Actions

Use the official `setup-jarvy` action:

```yaml
name: CI
on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Jarvy
        uses: Cliftonz/jarvy/.github/actions/setup-jarvy@main
        with:
          method: cargo       # or "path" to build from workspace

      - name: Provision tools
        run: jarvy setup --ci

      - name: Verify environment
        run: jarvy doctor --format json
```

### Action Inputs

| Input | Default | Description |
|-------|---------|-------------|
| `method` | `cargo` | Install method: `cargo` (from crates.io) or `path` (build locally) |
| `version` | latest | Jarvy version to install |
| `path` | `.` | Source path when `method=path` |
| `locked` | `true` | Honor Cargo.lock |

## GitLab CI

```yaml
stages:
  - setup
  - build

provision:
  stage: setup
  image: rust:latest
  script:
    - cargo install jarvy
    - jarvy setup --ci
    - jarvy doctor
```

## CircleCI

```yaml
version: 2.1

jobs:
  build:
    docker:
      - image: cimg/rust:1.85
    steps:
      - checkout
      - run:
          name: Install Jarvy
          command: cargo install jarvy
      - run:
          name: Provision tools
          command: jarvy setup --ci
```

## Azure Pipelines

```yaml
trigger:
  - main

pool:
  vmImage: ubuntu-latest

steps:
  - script: |
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      source $HOME/.cargo/env
      cargo install jarvy
    displayName: Install Jarvy

  - script: jarvy setup --ci
    displayName: Provision tools

  - script: jarvy --version
    displayName: Show Jarvy version
```

## Generate CI Configs

Jarvy can generate CI configuration files for you:

```bash
# Generate a GitHub Actions workflow
jarvy ci-config github

# Generate GitLab CI config
jarvy ci-config gitlab

# Preview without writing
jarvy ci-config circleci --dry-run
```

Supported providers: `github`, `gitlab`, `circleci`, `azure`, `bitbucket`, `travis`, `jenkins`, `buildkite`, `teamcity`, `appveyor`.

## CI Behavior

When Jarvy detects a CI environment (`CI=true`), it automatically:

- Switches to non-interactive mode (no prompts)
- Disables telemetry
- Disables auto-update checks
- Skips services auto-start (unless `start_in_ci = true`)

You can override these defaults:

```bash
# Force CI mode
jarvy setup --ci

# Force interactive mode in CI
jarvy setup --no-ci
```

## Drift Detection in CI

Use `jarvy drift check` as a CI gate to ensure environments match the config:

```yaml
# GitHub Actions example
- name: Check for drift
  run: jarvy drift check --format json
  # Exit code 1 = drift detected, fails the job
```

## CI Environment Info

```bash
# Show detected CI environment
jarvy ci-info
```

Output includes: provider name, build ID, branch, commit SHA, PR number (if applicable), and which environment variables were detected.

## Pre-commit Hook

Jarvy ships consumable [pre-commit](https://pre-commit.com) hooks that
validate `jarvy.toml` before every commit. Add to your
`.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/Cliftonz/jarvy
    rev: v0.5.2            # pin to a released tag
    hooks:
      - id: jarvy-validate           # runs: jarvy validate --strict
      # - id: jarvy-validate-json    # same, machine-readable JSON output
```

The hooks use `language: system`, so they run the `jarvy` binary you
already have installed (via `install.sh`, brew, cargo, or the
devcontainer feature below) — pre-commit does not build jarvy itself.
They trigger whenever a `jarvy.toml` changes and validate the repo-root
`./jarvy.toml`; monorepos with per-member configs can add a second entry
with `args: [--file, path/to/jarvy.toml]`.

## Devcontainer Feature

Install jarvy into a devcontainer / GitHub Codespace via the feature in
`dist/devcontainer/jarvy`:

```jsonc
{
  "features": {
    "ghcr.io/cliftonz/jarvy/jarvy:0": {
      "version": "latest",
      "channel": "stable",
      "runSetup": false
    }
  }
}
```

Options: `version` (release tag or `latest`), `channel`
(`stable`/`beta`/`nightly`), and `runSetup` (emit a postCreate script
that runs `jarvy setup` against `./jarvy.toml`). The install delegates to
the canonical `install.sh`, so the binary is checksum-verified like every
other channel. See `dist/devcontainer/jarvy/README.md` for details.
