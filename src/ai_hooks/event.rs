//! Canonical hook event taxonomy.
//!
//! Jarvy normalizes hook events across agents into a single enum, then
//! delegates per-agent name mapping to the [`agents`](crate::ai_hooks::agents)
//! module. Not every agent supports every event — providers return
//! [`AiHookError::UnsupportedEvent`](crate::ai_hooks::error::AiHookError) when
//! an entry maps onto an event the target agent cannot fire.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Fires before a tool call executes. Most common interception point;
    /// used by all `block-*` library hooks.
    PreToolUse,
    /// Fires after a tool call returns. Useful for logging / redaction.
    PostToolUse,
    /// Fires when the user submits a prompt — agent-specific support varies.
    UserPromptSubmit,
    /// Fires at session start (banner, env setup).
    SessionStart,
    /// Fires when the agent stops.
    Stop,
    /// Fires before context compaction.
    PreCompact,
    /// Fires before a shell/bash command specifically. Cursor + Windsurf
    /// expose this as a distinct event; Claude Code/Codex fold it into
    /// `PreToolUse` filtered to the Bash tool.
    PreShellExecution,
}

impl HookEvent {
    /// Every variant in a stable order. Used by integration tests and
    /// reserved for future CLI surface (e.g. `--event` filters).
    #[allow(dead_code)]
    pub const ALL: &'static [HookEvent] = &[
        HookEvent::PreToolUse,
        HookEvent::PostToolUse,
        HookEvent::UserPromptSubmit,
        HookEvent::SessionStart,
        HookEvent::Stop,
        HookEvent::PreCompact,
        HookEvent::PreShellExecution,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HookEvent::PreToolUse => "pre_tool_use",
            HookEvent::PostToolUse => "post_tool_use",
            HookEvent::UserPromptSubmit => "user_prompt_submit",
            HookEvent::SessionStart => "session_start",
            HookEvent::Stop => "stop",
            HookEvent::PreCompact => "pre_compact",
            HookEvent::PreShellExecution => "pre_shell_execution",
        }
    }
}

impl fmt::Display for HookEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_have_unique_strings() {
        let mut seen = std::collections::HashSet::new();
        for event in HookEvent::ALL {
            assert!(seen.insert(event.as_str()), "duplicate str for {event:?}");
        }
    }

    #[test]
    fn round_trip_through_serde() {
        for event in HookEvent::ALL {
            let json = serde_json::to_string(event).unwrap();
            let parsed: HookEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(*event, parsed);
        }
    }
}
