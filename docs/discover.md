# Auto-discovery (`jarvy discover`)

`jarvy discover` scans the project root for marker files and emits a
suggested `jarvy.toml` so new contributors don't have to guess what
tools the project needs. Drop into any repo and run it — the output is
either a printed suggestion list (default) or a merge straight into
`jarvy.toml` (`--apply`).

> **PRD-044 MVP.** Built-in rules cover the most common ecosystems
> jarvy ships handlers for today (rust, node, python, go, ruby, docker,
> kubectl, helm, terraform, pre-commit, make, just). Custom rule files
> are intentionally deferred — adding a new ecosystem today is one
> entry in `src/discover/rules.rs::default_rules()`.

## Quick start

```bash
cd ~/work/some-project
jarvy discover
```

```text
Project Analysis
================

Detected Technologies:
  rust       1.85.0     (from rust-toolchain.toml)
  docker     latest     (from Dockerfile)
  kubectl    latest     (from k8s)
  pre-commit latest     (from .pre-commit-config.yaml)

Required (would be added):
  rust = "1.85.0"   # detected from rust-toolchain.toml
  docker = "latest"  # detected from Dockerfile

Recommended companions:
  cargo-watch = "latest"   # commonly used with rust
  helm = "latest"          # commonly used with kubectl
  k9s = "latest"           # commonly used with kubectl
  pre-commit = "latest"    # commonly used with pre-commit-config

Run `jarvy discover --apply` to update jarvy.toml.
```

## Flags

| Flag | Behavior |
|------|---------|
| `--file <path>` | Path to the jarvy.toml to read / update. Defaults to `./jarvy.toml`. |
| `--apply` | Write suggestions into the file. Creates it if missing; merges if it exists. |
| `--missing` | Plain `name = "version"` lines only (one per row). Machine-readable but easier to eyeball than JSON. |
| `--format json` | Full report (detections + required + recommended + already-configured) as JSON. |

## Trust posture

`jarvy discover` is dry-run by default. `--apply` is opt-in and the
merge is **append-only**:

- Tools already present under `[provisioner]` are left exactly as
  pinned. A hand-curated `rust = "1.84.0"` survives even when
  rust-toolchain.toml says `1.85.0`.
- New entries are inserted at the end of the existing `[provisioner]`
  block (before the next `[section]`), preserving every comment and
  ordering choice above the insertion point.
- If `[provisioner]` doesn't exist, it's appended at the end of the
  file with a `# Added by jarvy discover` comment.

If the merge would change nothing (no new tools), the file is left
byte-identical.

## Detection rule shape

Each entry in `src/discover/rules.rs::default_rules()` is:

```rust
DetectionRule {
    name: "rust",            // canonical jarvy tool name
    detect: vec![
        File { file: "Cargo.toml" },
        File { file: "rust-toolchain.toml" },
        // ...
    ],
    version_from: Some(VersionSource {
        file: "rust-toolchain.toml",
        pattern: Some(r#"channel\s*=\s*"([^"]+)""#),
    }),
    suggests: vec!["cargo-watch", "cargo-nextest"],
    category: Runtime,
}
```

The matcher walks only the project root (no subdir descent). This
keeps detection fast on large repos and avoids vendored / `node_modules`
false positives. Add a new ecosystem by appending one entry — no other
code changes needed.

## What's deferred

Items from PRD-044 that v1 does NOT ship:

- `--rules <file>` for custom rule files
- `--interactive` confirmation flow
- `FileContaining` patterns (only `File` / `Dir` + `*.ext` globs today)
- Continuous discovery during `jarvy setup`

Track these in the original PRD; open an issue if you need one.

## Telemetry

No telemetry events specific to discover. The command is read-only by
default and `--apply` writes a local file — neither path needs an
OTEL breadcrumb beyond the standard CLI command tracing.
