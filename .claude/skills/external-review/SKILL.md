---
name: external-review
description: Well-rounded post-implementation review — an independent second-pass by GPT via the locally installed Codex CLI PLUS the /parallel-code-review multi-persona pass, merged into one report. Only call this AFTER an implementation is complete (never mid-implementation); use when the user asks for an external review, codex review, second opinion, or independent perspective on uncommitted changes, a branch diff, or a specific commit — and as the reviewer pass before merging per the model-routing rules in CLAUDE.md. Read-only; never applies fixes.
---

# External Review (Codex + parallel-code-review)

Precondition: the implementation under review is DONE. Do not invoke this
skill on in-progress work — finish first, review second.

Two review passes run side by side, then merge:

1. **Codex pass** — independent external perspective (commands below).
2. **`parallel-code-review` pass** — invoke the `parallel-code-review` skill
   via the Skill tool on the same target (five parallel expert personas:
   performance, security, QA/testing, observability, maintainability).

Launch the codex command in the background, run `parallel-code-review` while
it executes, then collect both.

## Codex commands

Shell out to the Codex CLI. Do NOT guess flags — use exactly these commands
(verified against codex-cli 0.135.0):

```bash
# Uncommitted changes (staged + unstaged + untracked)
codex exec review --uncommitted -o "$SCRATCH/codex-review.txt" "<one-sentence focus, optional>"

# Branch diff vs a base branch — NOTE: --base cannot be combined with a
# focus prompt (clap conflict, verified codex-cli 0.135.0); run it bare
codex exec review --base main -o "$SCRATCH/codex-review.txt"

# A single commit
codex exec review --commit <sha> -o "$SCRATCH/codex-review.txt" "<one-sentence focus, optional>"
```

`$SCRATCH` = the session scratchpad directory. `-o` writes the agent's final
message to that file; read it after the command exits rather than parsing the
streamed output.

## Rules

- Keep the codex prompt simple. One or two sentences of focus at most ("focus
  on the trust-boundary checks in src/library_registry"). Do not prompt Codex
  like it's Claude — no elaborate role/format instructions.
- Run each pass exactly once per review target. Do not silently rerun.
- Merge the two passes into one report: findings both reviewers agree on go
  first (highest confidence), then unique findings labeled by source
  (`codex` / `parallel-code-review`). Dedupe by file+issue.
- Report the merged findings back up faithfully. If neither pass finds
  anything, say that clearly AND name the review target inspected (e.g.
  "codex and parallel-code-review both reviewed the branch diff vs main and
  reported no findings") so the parent session doesn't get confused and rerun.
- This skill is read-only. Never let it apply fixes — the caller decides what
  to do with findings.
- Inside a Workflow, run this via a Sonnet low-effort proxy sub-agent labeled
  `codex-proxy:<target>` (workflows can only spawn Claude models directly).
