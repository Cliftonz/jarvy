---
title: "JSON Schema for jarvy.toml — Jarvy"
description: "Wire up VS Code, JetBrains IDEs, or Helix to autocomplete and validate jarvy.toml inline using the published JSON Schema."
tags:
  - reference
  - schema
  - editor
---

# JSON Schema for `jarvy.toml`

Jarvy publishes a JSON Schema for `jarvy.toml`. Wire it into your editor and you get inline autocomplete, hover descriptions, and live validation while you edit.

**Schema URL:**

```text
https://jarvy.dev/schema/jarvy.schema.json
```

---

## VS Code

Install the [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml) extension, then add to your settings:

```json title=".vscode/settings.json"
{
  "evenBetterToml.schema.associations": {
    "jarvy.toml": "https://jarvy.dev/schema/jarvy.schema.json"
  }
}
```

Or use the inline schema directive at the top of `jarvy.toml` (works without any settings):

```toml title="jarvy.toml"
#:schema https://jarvy.dev/schema/jarvy.schema.json

[provisioner]
node = "20"
```

You'll get:

- Autocomplete for `[provisioner]`, `[hooks]`, `[env.vars]`, every block
- Inline hover hints with the description from the schema
- Squiggly underlines on unknown sections, invalid version syntax, missing required fields

---

## JetBrains (IntelliJ, WebStorm, GoLand, RustRover, …)

JetBrains has built-in JSON Schema support for TOML. Add the schema in **Settings → Languages & Frameworks → Schemas and DTDs → JSON Schema Mappings**:

| Field | Value |
|---|---|
| **Name** | `jarvy.toml` |
| **Schema file or URL** | `https://jarvy.dev/schema/jarvy.schema.json` |
| **Schema version** | JSON Schema version 7 |
| **File path pattern** | `jarvy.toml` |

Or per-project in `.idea/jsonSchemas.xml` if you want to commit the binding.

---

## Helix / Neovim (LSP)

Use [taplo](https://taplo.tamasfe.dev/) as a TOML language server with the schema:

```toml title=".taplo.toml"
[[schema.rule]]
url = "https://jarvy.dev/schema/jarvy.schema.json"
formats = ["toml"]
include = ["**/jarvy.toml"]
```

Then start your editor with `taplo` configured as the TOML LSP. You'll get hover, completion, and diagnostics through any LSP-aware editor.

---

## Offline / pinned versions

If your team is air-gapped or wants to pin to a specific Jarvy version:

```bash
curl -fsSL https://jarvy.dev/schema/jarvy.schema.json -o .jarvy/jarvy.schema.json
```

Then point the schema URL at the local file:

```toml
#:schema ./.jarvy/jarvy.schema.json
```

Commit `.jarvy/jarvy.schema.json` and refresh on Jarvy upgrades.

---

## What the schema covers

The published schema describes every TOML section Jarvy accepts:

- `[provisioner]` — tool entries with version specs
- `[privileges]` — sudo behavior
- `[hooks]` — global + per-tool hooks
- `[env.vars]`, `[env.secrets]` — environment
- `[services]` — Docker Compose / Tilt integration
- `[roles.*]` — role definitions and inheritance
- `[npm]`, `[pip]`, `[cargo]` — language packages
- `[git]` — Git configuration
- `[network]` — proxy + TLS
- `[drift]` — baseline + version policy
- `[telemetry]`, `[update]`, `[logging]`, `[commands]`

Each field has a description, type, default value, and (where applicable) an enum constraint.

---

## What's *not* covered

The schema validates **shape**, not **semantics**:

- It accepts any string for a tool name. `jarvy validate` checks the registry; the schema can't.
- Version strings are validated by length, not by parsing. `jarvy validate` does the SemVer check.
- Role inheritance cycles are runtime checks, not schema rules.

So the editor catches typos and unknown fields; `jarvy validate` catches everything else. Run both.

---

## Reporting schema issues

The schema is hand-maintained alongside the Rust config struct. If you see a field accepted by `jarvy validate` that the schema rejects (or vice versa), [open an issue](https://github.com/Cliftonz/jarvy/issues) — schema drift is a bug.
