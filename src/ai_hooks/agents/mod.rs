//! Per-agent provisioners.
//!
//! Each agent owns a small implementation of [`AgentProvisioner`] that
//! resolves its on-disk settings path, parses any existing config, and
//! merges Jarvy-managed entries while leaving foreign entries untouched.
//!
//! Why a trait instead of one big match: the hook config schemas diverge
//! enough (Claude's `hooks: { Event: [{matcher, hooks: [...] }] }` vs.
//! Cursor's `hooks: { event: [{command, ...}] }` vs. Cline's
//! filesystem-as-hooks layout vs. Continue's YAML deny-list) that a flat
//! emitter would balloon. The trait keeps each agent self-contained and
//! makes it cheap to add the next one.

use std::borrow::Cow;
use std::path::PathBuf;

use crate::ai_hooks::config::{AgentTarget, HookScope};
use crate::ai_hooks::error::AiHookError;

pub mod claude_code;
pub mod cline;
pub mod codex;
pub mod continue_dev;
pub mod cursor;
pub(crate) mod io;
pub mod json_merge;
pub mod markers;
pub mod windsurf;

/// Result of a successful [`AgentProvisioner::apply`] call.
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub agent: &'static str,
    pub path: PathBuf,
    pub applied: usize,
    pub warnings: Vec<String>,
}

/// Result of a successful [`AgentProvisioner::remove`] call.
#[derive(Debug, Clone)]
pub struct RemoveOutcome {
    pub agent: &'static str,
    pub path: PathBuf,
    pub removed: usize,
    /// Entries on disk that carried our marker key but whose hash did
    /// not match — preserved by the impersonation defense. Currently
    /// always 0 (remove sweeps every marker entry; the impersonation
    /// defense lives in `apply`); reserved for future hash-aware
    /// removal modes.
    #[allow(dead_code)]
    pub foreign_preserved: usize,
    /// Per-agent warnings surfaced during removal (e.g. "settings file
    /// vanished mid-run"). Empty in steady state.
    #[allow(dead_code)]
    pub warnings: Vec<String>,
}

/// Result of a [`AgentProvisioner::check`] call — drift between desired
/// state and on-disk state.
#[derive(Debug, Clone, Default)]
pub struct CheckOutcome {
    pub agent: &'static str,
    pub path: PathBuf,
    pub missing: Vec<String>,
    pub extra_jarvy: Vec<String>,
}

impl CheckOutcome {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.extra_jarvy.is_empty()
    }
}

/// Per-agent provisioning contract.
pub trait AgentProvisioner: Sync {
    /// Stable telemetry / log identifier — matches `AgentTarget::slug()`.
    /// Read by tests; production code reaches the slug via the
    /// `ApplyOutcome` / `RemoveOutcome` field returned from the trait.
    #[allow(dead_code)]
    fn slug(&self) -> &'static str;

    /// Where the agent stores its hook config for `scope`.
    fn settings_path(&self, scope: HookScope) -> Result<PathBuf, AiHookError>;

    /// Write `entries` to disk, merging with whatever is already there.
    fn apply(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<ApplyOutcome, AiHookError>;

    /// Compare desired state vs. on-disk state without mutating.
    fn check(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<CheckOutcome, AiHookError>;

    /// Strip Jarvy-managed entries.
    fn remove(&self, scope: HookScope) -> Result<RemoveOutcome, AiHookError>;
}

/// A hook entry expanded by the runner into its concrete command, ready
/// for a provisioner to serialize. Library lookup, custom-command audit,
/// and Windows translation all happen before this struct is built.
///
/// The lifetime `'a` lets library hooks borrow their `&'static str`
/// bodies straight from `library::LIBRARY` instead of cloning. Custom
/// commands store owned `String`s in the `Cow::Owned` variant.
#[derive(Debug, Clone)]
pub struct ResolvedEntry<'a> {
    /// Stable identifier — used as the `_jarvy_managed` marker.
    pub name: String,
    /// Library hook name, when the entry resolved from one. Used for
    /// telemetry + report classification.
    pub library_source: Option<String>,
    pub event: crate::ai_hooks::event::HookEvent,
    pub matcher: Option<String>,
    /// Bash / sh script for Unix-y agents.
    pub bash_command: Cow<'a, str>,
    /// Pre-resolved PowerShell command (native or auto-translated).
    pub windows_command: Cow<'a, str>,
    /// Whether the Windows variant was auto-translated and should warn.
    pub windows_warned: bool,
    pub timeout_ms: u64,
}

/// Stateless static instances of each provisioner — all zero-sized, so
/// `&'static dyn AgentProvisioner` is free.
static CLAUDE_CODE: claude_code::ClaudeCodeProvisioner = claude_code::ClaudeCodeProvisioner;
static CURSOR: cursor::CursorProvisioner = cursor::CursorProvisioner;
static CODEX: codex::CodexProvisioner = codex::CodexProvisioner;
static WINDSURF: windsurf::WindsurfProvisioner = windsurf::WindsurfProvisioner;
static CLINE: cline::ClineProvisioner = cline::ClineProvisioner;
static CONTINUE: continue_dev::ContinueProvisioner = continue_dev::ContinueProvisioner;

pub fn provisioner_for(target: AgentTarget) -> &'static dyn AgentProvisioner {
    match target {
        AgentTarget::ClaudeCode => &CLAUDE_CODE,
        AgentTarget::Cursor => &CURSOR,
        AgentTarget::Codex => &CODEX,
        AgentTarget::Windsurf => &WINDSURF,
        AgentTarget::Cline => &CLINE,
        AgentTarget::Continue => &CONTINUE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provisioner_for_returns_matching_slug() {
        for target in AgentTarget::ALL {
            let p = provisioner_for(*target);
            assert_eq!(p.slug(), target.slug());
        }
    }
}
