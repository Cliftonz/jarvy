//! AI agent hook provisioning (PRD: AI Hooks).
//!
//! Distributes hook configurations across heterogeneous AI coding agents
//! (Claude Code, Cursor, Codex CLI, Windsurf, Cline, Continue) so that a
//! team can ship guard rails — e.g. block `rm -rf`, block `git push --force`,
//! block secret-bearing commits — through a single `jarvy.toml` section.
//!
//! # Architecture
//!
//! - [`config`]   — Public `[ai_hooks]` schema (`AiHooksConfig`, `HookEntry`).
//! - [`event`]    — Canonical event taxonomy + per-agent event mapping.
//! - [`library`]  — Built-in, curated hook scripts (`block-rm-rf`, ...).
//! - [`agents`]   — `AgentProvisioner` trait + per-agent implementations.
//! - [`runner`]   — Top-level `apply` / `check` / `remove` orchestration.
//! - [`platform`] — Host detection + EncodedCommand PowerShell wrap.
//! - [`error`]    — `AiHookError` type with stable `kind()` tags.
//!
//! # Trust model
//!
//! Library hooks are vetted Jarvy source. Custom hook entries (raw
//! `command` fields in `jarvy.toml`) run arbitrary shell with the user's
//! privileges — they are flagged by [`runner::audit_custom_commands`]
//! and require BOTH `allow_custom_commands = true` AND the config to be
//! [`config::ConfigOrigin::Local`] (i.e. not fetched via
//! `jarvy setup --from <url>`). The Remote origin tag is set by the
//! loader; a poisoned team `jarvy.toml` cannot lift the gate on its own.

pub mod agents;
pub mod config;
pub mod error;
pub mod event;
pub mod library;
pub mod platform;
pub mod runner;

#[allow(unused_imports)]
pub use config::{AgentTarget, AiHooksConfig, ConfigOrigin, HasOrigin, HookEntry, HookScope};
#[allow(unused_imports)]
pub use error::AiHookError;
#[allow(unused_imports)]
pub use event::HookEvent;
#[allow(unused_imports)]
pub use runner::{ApplyReport, RemoveReport, apply, check, remove};
