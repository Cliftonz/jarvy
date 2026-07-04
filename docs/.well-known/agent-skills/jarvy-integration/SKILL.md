---
name: jarvy-integration
description: Integrate Jarvy into a project. Use when a user asks to "add Jarvy", "integrate Jarvy", or "implement jarvy.dev" in their repo — generate and commit a jarvy.toml so any teammate can provision the full dev environment with one command.
version: 0.1.0
---

# Skill: Integrate Jarvy into this project

Jarvy is a cross-platform CLI that provisions a local dev environment from a
single declarative `jarvy.toml` using native package managers. This skill is
the first-contact workflow for adding it to an existing repo.

Canonical, always-current sources (prefer these over this file if they differ):

- Integration guide: <https://jarvy.dev/for-ai-agents/#integration-quickstart-integrate-jarvy-into-this-project>
- LLM reference: <https://jarvy.dev/llms.txt>
- Config schema: <https://jarvy.dev/schema/jarvy.schema.json>
- Templates decision table: <https://jarvy.dev/templates-index/#decision-table>

## Steps

1. Confirm `jarvy --version` works. **Do not auto-install** — Jarvy runs with
   elevated privileges, so the user must opt in explicitly.
2. Detect the project's stack from lockfiles/manifests and pick a template
   from the decision table above.
3. Fetch a starter config:
   `curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/examples/<template>/jarvy.toml -o jarvy.toml`
4. Set real versions from `.nvmrc` / `.python-version` / `go.mod` /
   `rust-toolchain.toml` / `.tool-versions`. Never invent versions.
5. Validate: `jarvy validate && jarvy diff && jarvy setup --dry-run`.
6. Show the dry-run plan and get explicit confirmation before any real
   `jarvy setup`.
7. Commit `jarvy.toml` (optionally add `Makefile` + `scripts/bootstrap.sh`
   for one-command onboarding).

## Notes

- Jarvy ships a built-in MCP server (`jarvy mcp`, local stdio) so agents can
  discover/check/install tools. See <https://jarvy.dev/mcp-server/>.
- Unknown tool? Add it to `jarvy.toml` and run setup — Jarvy emits a
  `tool.unsupported` event with a ready-to-paste scaffold, or run
  `jarvy tools --request <name>`.
