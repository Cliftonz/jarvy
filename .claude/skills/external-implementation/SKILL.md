---
name: external-implementation
description: Delegate a bounded, clearly-specified implementation task to GPT via the locally installed Codex CLI, usually in a git worktree. Use for mechanical clear-spec work where taste doesn't matter — migrations, boilerplate, repetitive refactors, test scaffolding, mass renames — per the model-routing rules in CLAUDE.md. Claude reviews the result before anything merges.
---

# External Implementation (Codex)

Shell out to the Codex CLI for bulk mechanical work. Do NOT guess flags — use
exactly these commands (verified against codex-cli 0.135.0):

```bash
# 1. Isolate in a worktree (default; skip only for trivial single-file edits)
git -C /Users/zacclifton/RustroverProjects/Jarvy worktree add "$SCRATCH/wt-<task>" -b codex/<task>

# 2. Run codex write-capable inside it
codex exec -s workspace-write -C "$SCRATCH/wt-<task>" \
  -o "$SCRATCH/codex-impl.txt" \
  "<task spec — plain, complete, self-contained>"

# 3. Clean up when done (after merging or abandoning)
git -C /Users/zacclifton/RustroverProjects/Jarvy worktree remove "$SCRATCH/wt-<task>"
```

`$SCRATCH` = the session scratchpad directory.

## Task-spec rules

- The spec must be complete and self-contained: exact files or file patterns,
  the required behavior, and the acceptance check (e.g. "cargo test
  --all-features passes"). Codex won't ask follow-ups in exec mode.
- Keep the prompt plain — no Claude-style role/format scaffolding.
- Only delegate work with a clear spec and low taste requirement. If the task
  needs API-design or naming judgment, keep it on Fable/Opus.

## After codex returns

1. Read `$SCRATCH/codex-impl.txt`, then review the diff yourself
   (`git -C <worktree> diff`) — or run the `external-review` skill on it for
   an independent pass.
2. Verify in the worktree: `cargo fmt --all`, `cargo clippy --all-features -- -D warnings`,
   `cargo test --all-features` (this repo's CI gates).
3. Report outcome faithfully: what was asked, what changed, what passed/failed.
   If the output misses the bar, redo it with a smarter model without asking —
   defaults, not limits.
