//! Codex CLI (OpenAI) provisioner.
//!
//! Writes `~/.codex/hooks.json` (user) or `<project>/.codex/hooks.json`
//! (project). Codex's schema is a flat array of entries with both a
//! `command` and `commandWindows` field — agent picks at runtime.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::io::home_or_err;
use super::json_merge::{collect_marker_names, entry_hash, retain_non_jarvy_named};
use super::markers::{JSON_HASH_KEY, JSON_MARKER_KEY};
use super::{AgentProvisioner, ApplyOutcome, CheckOutcome, RemoveOutcome, ResolvedEntry};
use crate::ai_hooks::config::HookScope;
use crate::ai_hooks::error::AiHookError;
use crate::ai_hooks::event::HookEvent;

pub struct CodexProvisioner;

impl CodexProvisioner {
    const SLUG: &'static str = "codex";

    fn event_key(event: HookEvent) -> &'static str {
        match event {
            HookEvent::PreToolUse | HookEvent::PreShellExecution => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::SessionStart => "SessionStart",
            HookEvent::Stop => "Stop",
            HookEvent::PreCompact => "PreCompact",
        }
    }

    fn entry_to_json(entry: &ResolvedEntry<'_>) -> Value {
        let hash = entry_hash(entry);
        json!({
            JSON_MARKER_KEY: entry.name,
            JSON_HASH_KEY: hash,
            "event": Self::event_key(entry.event),
            "matcher": entry.matcher.clone().unwrap_or_else(|| "*".to_string()),
            "handler": {
                "type": "command",
                "command": entry.bash_command.as_ref(),
                "commandWindows": entry.windows_command.as_ref(),
                "timeoutMs": entry.timeout_ms,
            }
        })
    }
}

impl AgentProvisioner for CodexProvisioner {
    fn slug(&self) -> &'static str {
        Self::SLUG
    }

    fn settings_path(&self, scope: HookScope) -> Result<PathBuf, AiHookError> {
        match scope {
            HookScope::User => Ok(home_or_err()?.join(".codex").join("hooks.json")),
            HookScope::Project => Ok(PathBuf::from(".codex").join("hooks.json")),
        }
    }

    fn apply(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<ApplyOutcome, AiHookError> {
        let path = self.settings_path(scope)?;
        let mut root = super::io::read_or_default_object(&path)?;
        let list = root
            .entry("hooks")
            .or_insert_with(|| Value::Array(Vec::new()));
        let Value::Array(list) = list else {
            return Err(AiHookError::InvalidEntry {
                name: "hooks".to_string(),
                reason: "existing `hooks` is not an array".to_string(),
            });
        };

        let mut warnings = Vec::new();
        let mut applied = 0usize;
        for entry in entries {
            retain_non_jarvy_named(list, &entry.name);
            list.push(Self::entry_to_json(entry));
            if entry.windows_warned {
                warnings.push(format!(
                    "{}: no command_windows supplied — Windows execution stubbed",
                    entry.name
                ));
            }
            applied += 1;
        }
        super::io::write_json(&path, &Value::Object(root))?;
        Ok(ApplyOutcome {
            agent: Self::SLUG,
            path,
            applied,
            warnings,
        })
    }

    fn check(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<CheckOutcome, AiHookError> {
        let path = self.settings_path(scope)?;
        let root = super::io::read_or_default_object(&path)?;
        let mut outcome = CheckOutcome {
            agent: Self::SLUG,
            path,
            ..CheckOutcome::default()
        };
        let list = match root.get("hooks") {
            Some(Value::Array(arr)) => arr,
            _ => {
                outcome.missing = entries.iter().map(|e| e.name.clone()).collect();
                return Ok(outcome);
            }
        };

        let on_disk = collect_marker_names(list);
        let desired: std::collections::HashSet<_> =
            entries.iter().map(|e| e.name.as_str()).collect();
        let actual: std::collections::HashSet<_> = on_disk.iter().map(String::as_str).collect();
        outcome.missing = desired.difference(&actual).map(|s| s.to_string()).collect();
        outcome.extra_jarvy = actual.difference(&desired).map(|s| s.to_string()).collect();
        outcome.missing.sort();
        outcome.extra_jarvy.sort();
        Ok(outcome)
    }

    fn remove(&self, scope: HookScope) -> Result<RemoveOutcome, AiHookError> {
        let path = self.settings_path(scope)?;
        let mut root = super::io::read_or_default_object(&path)?;
        let mut removed = 0usize;
        if let Some(Value::Array(arr)) = root.get_mut("hooks") {
            let before = arr.len();
            arr.retain(|v| v.get(JSON_MARKER_KEY).is_none());
            removed += before - arr.len();
        }
        super::io::write_json(&path, &Value::Object(root))?;
        Ok(RemoveOutcome {
            agent: Self::SLUG,
            path,
            removed,
            foreign_preserved: 0,
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn entry_serializes_with_handler_block() {
        let entry = ResolvedEntry {
            name: "block-rm-rf".to_string(),
            library_source: Some("block-rm-rf".to_string()),
            event: HookEvent::PreToolUse,
            matcher: Some("Bash".to_string()),
            bash_command: Cow::Borrowed("exit 0"),
            windows_command: Cow::Borrowed("exit 0"),
            windows_warned: false,
            timeout_ms: 5_000,
        };
        let v = CodexProvisioner::entry_to_json(&entry);
        assert_eq!(v["handler"]["type"], "command");
        assert_eq!(v["event"], "PreToolUse");
        assert_eq!(v[JSON_MARKER_KEY], "block-rm-rf");
        assert_eq!(v[JSON_HASH_KEY].as_str().unwrap().len(), 64);
    }
}
