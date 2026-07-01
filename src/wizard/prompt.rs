//! System-prompt assembly for headless-mode agent invocations.
//!
//! The prompt has two branches that differ only in the opening
//! instruction: greenfield (no `jarvy.toml`) vs refinement
//! (`jarvy.toml` already present). Both branches share the same MCP
//! tool inventory and the same hard rules — only the first paragraph
//! changes — so we render from a single template with a small
//! variable section up top.

use super::context::ProjectContext;
use crate::agents::Agent;

/// Build the full system + user prompt the headless agent will see.
///
/// Format: opening branch + project envelope (JSON) + canonical
/// tool-call playbook + hard rules. Mirrors the skill body so a user
/// who hits both modes sees identical guidance.
pub fn build(ctx: &ProjectContext, agent: Agent) -> String {
    let mut out = String::new();
    out.push_str(opening(ctx));
    out.push_str("\n\n## Project context (JSON)\n\n```json\n");
    out.push_str(
        &serde_json::to_string_pretty(ctx)
            .unwrap_or_else(|_| "{\"error\":\"context_serialize_failed\"}".to_string()),
    );
    out.push_str("\n```\n\n");
    out.push_str(PLAYBOOK);
    out.push_str("\n\n");
    out.push_str(&hard_rules(agent));
    out
}

/// Opening instruction. Greenfield gets the bootstrap nudge;
/// refinement gets the merge-don't-replace nudge. Both branches
/// assume the user already typed `jarvy wizard --apply` — that IS
/// the approval. The spawned agent runs in single-turn non-interactive
/// mode (`claude -p` / `codex exec`), so there is no opportunity to
/// ask the user a follow-up question; do the work end-to-end and
/// summarise.
fn opening(ctx: &ProjectContext) -> &'static str {
    if ctx.has_jarvy_toml {
        "You are refining an existing Jarvy configuration. The project already has a \
`jarvy.toml`. Read it (via the MCP server's read tools), compare to the discover \
output, and merge missing tools into the existing [provisioner] table via \
jarvy_discover_apply (apply=true). Do NOT replace the file. The user already \
approved by typing `jarvy wizard --apply`. You will NOT get a follow-up turn — \
execute the playbook end-to-end and print a final summary."
    } else {
        "You are bootstrapping a fresh Jarvy setup for a project that has no jarvy.toml \
yet. Call jarvy_discover_apply once with apply=true to write a starter jarvy.toml \
covering every detected ecosystem. The user already approved by typing \
`jarvy wizard --apply`. You are running non-interactively (single turn) — DO NOT \
ask the user questions; execute the playbook end-to-end and print a final summary \
the user can read."
    }
}

/// MCP tool playbook — identical across greenfield + refinement.
/// Designed for single-turn execution (`claude -p` / `codex exec`):
/// no approval gates, no clarifying questions, no second turn.
const PLAYBOOK: &str = "\
## Step playbook (idempotent — same inputs MUST produce same outputs)

You are running non-interactively. Execute every step in order. Do NOT
stop to ask the user anything — they already typed
`jarvy wizard --apply`, which is the approval.

1. Call `jarvy_wizard_plan` first. Read-only; returns the proposed
   plan as JSON. Use it to decide what tools to write.
2. If `required` is empty AND `recommended` is empty, stop — the
   project is already configured. Skip steps 3-6 and print a one-line
   summary.
3. Call `jarvy_discover_apply` ONCE with `apply=true` to write/merge
   tools into `[provisioner]`. The tool rate-limits and audit-logs.
   Repeated calls with the same input return `target = \"noop\"` —
   if you see that, move on, don't retry.
4. Skip `jarvy_ai_hooks_apply` unless the project context obviously
   needs hooks (e.g. lint-on-save for a TypeScript repo). Default is
   to leave hooks for the user to opt into later.
5. Skip `jarvy_mcp_register_apply` — that's a separate explicit step.
6. End with `jarvy_validate_config` and print the summary.

Do NOT loop these tools waiting for a different outcome — they are
idempotent by design, and Jarvy's rate limiter will reject repeated
mutating calls within the cooldown window. If a call no-ops, that is
the terminal state.

Do NOT run `jarvy setup` for the user; running install commands is a
separate, explicit step the user types themselves.

Final output: a short markdown summary of (a) what was written to
`jarvy.toml`, (b) the `jarvy_validate_config` result, (c) suggested
next commands (`jarvy setup`, etc.). Print it as your last response.
";

/// Per-agent hard rules. The body is the same; only the agent name
/// in the closing line varies so each agent sees its own header.
fn hard_rules(agent: Agent) -> String {
    format!(
        "## Hard rules\n\
\n\
- Never modify files outside the project root.\n\
- Never write secrets into jarvy.toml. If you find an API key or token, \
stop and tell the user to use [secrets] or an external manager.\n\
- Don't suggest tools jarvy can't install — the discover `uninstallable` \
bucket lists them.\n\
- Don't loop mutating MCP tools silently; each call audit-logs.\n\
- If the project's jarvy.toml was loaded from a remote source, refuse to \
auto-apply and explain why.\n\
\n\
Agent: {}\n",
        agent.slug()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::DiscoverReport;
    use std::path::PathBuf;

    fn ctx(has_toml: bool) -> ProjectContext {
        ProjectContext {
            project_dir: ".".to_string(),
            has_jarvy_toml: has_toml,
            top_level: vec![],
            markers: vec![],
            git: None,
            discover: DiscoverReport::default(),
        }
    }

    #[test]
    fn greenfield_branch_calls_for_bootstrap() {
        let p = build(&ctx(false), Agent::ClaudeCode);
        assert!(
            p.contains("no jarvy.toml") && p.contains("apply=true"),
            "greenfield prompt must instruct the agent to bootstrap a fresh jarvy.toml"
        );
    }

    #[test]
    fn refinement_branch_calls_for_merge() {
        let p = build(&ctx(true), Agent::ClaudeCode);
        assert!(
            p.contains("merge missing tools") && p.contains("Do NOT replace"),
            "refinement prompt must instruct the agent to merge, not replace"
        );
    }

    #[test]
    fn both_branches_skip_approval_gate() {
        // `claude -p` / `codex exec` are single-turn — there's no
        // follow-up turn for the user to approve. The prompt must
        // tell the agent the user already approved by typing
        // `jarvy wizard --apply` and to execute end-to-end without
        // asking questions, or the agent prints a plan and exits
        // without writing `jarvy.toml`.
        for has_toml in [false, true] {
            let p = build(&ctx(has_toml), Agent::ClaudeCode);
            assert!(
                p.contains("already approved") && p.contains("non-interactively"),
                "playbook must signal pre-approval + single-turn execution \
                 (has_jarvy_toml={has_toml})"
            );
            assert!(
                !p.contains("If the user approves"),
                "stale conversational approval gate leaked into prompt \
                 (has_jarvy_toml={has_toml})"
            );
        }
    }

    #[test]
    fn both_branches_share_playbook() {
        let g = build(&ctx(false), Agent::ClaudeCode);
        let r = build(&ctx(true), Agent::ClaudeCode);
        assert!(g.contains("jarvy_wizard_plan"));
        assert!(r.contains("jarvy_wizard_plan"));
        assert!(g.contains("jarvy_validate_config"));
        assert!(r.contains("jarvy_validate_config"));
    }

    #[test]
    fn includes_agent_name_in_hard_rules() {
        let p = build(&ctx(false), Agent::Codex);
        assert!(p.contains("Agent: codex"));
    }

    #[test]
    fn no_path_leak_when_project_dir_is_dot() {
        let mut c = ctx(false);
        c.project_dir = ".".to_string();
        let p = build(&c, Agent::ClaudeCode);
        assert!(!p.contains("/Users/"));
        assert!(!p.contains("/home/"));
    }

    // Keep PathBuf in scope so future expansion has the import handy.
    #[allow(dead_code)]
    fn _ensure_pathbuf() -> PathBuf {
        PathBuf::new()
    }
}
