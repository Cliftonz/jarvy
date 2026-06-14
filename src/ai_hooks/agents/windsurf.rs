//! Windsurf / Cascade provisioner.
//!
//! Writes `~/.codeium/windsurf/hooks.json` (user) or `.windsurf/hooks.json`
//! (project). Windsurf uses `command` (bash -c on Unix) + `powershell` for
//! Windows in the same entry — first-class cross-platform support, no shim
//! required. Exit code 2 from a pre-hook blocks the tool call.

use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::io::home_or_err;
use super::json_merge::{collect_marker_names, entry_hash, retain_non_jarvy_named};
use super::markers::{JSON_HASH_KEY, JSON_MARKER_KEY};
use super::{AgentProvisioner, ApplyOutcome, CheckOutcome, RemoveOutcome, ResolvedEntry};
use crate::ai_hooks::config::HookScope;
use crate::ai_hooks::error::AiHookError;
use crate::ai_hooks::event::HookEvent;

pub struct WindsurfProvisioner;

impl WindsurfProvisioner {
    const SLUG: &'static str = "windsurf";

    fn event_key(event: HookEvent) -> Result<&'static str, AiHookError> {
        Ok(match event {
            HookEvent::PreToolUse | HookEvent::PreShellExecution => "pre_run_command",
            HookEvent::PostToolUse => "post_run_command",
            HookEvent::UserPromptSubmit => "pre_user_prompt",
            HookEvent::SessionStart => "post_setup_worktree",
            HookEvent::Stop => "post_cascade_response",
            HookEvent::PreCompact => {
                return Err(AiHookError::UnsupportedEvent {
                    agent: "windsurf",
                    event: event.to_string(),
                });
            }
        })
    }

    fn entry_to_json(entry: &ResolvedEntry<'_>) -> Value {
        let hash = entry_hash(entry);
        json!({
            JSON_MARKER_KEY: entry.name,
            JSON_HASH_KEY: hash,
            "command": entry.bash_command.as_ref(),
            "powershell": entry.windows_command.as_ref(),
            "show_output": false,
        })
    }
}

impl AgentProvisioner for WindsurfProvisioner {
    fn slug(&self) -> &'static str {
        Self::SLUG
    }

    fn settings_path(&self, scope: HookScope) -> Result<PathBuf, AiHookError> {
        match scope {
            HookScope::User => Ok(home_or_err()?
                .join(".codeium")
                .join("windsurf")
                .join("hooks.json")),
            HookScope::Project => Ok(PathBuf::from(".windsurf").join("hooks.json")),
        }
    }

    fn apply(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<ApplyOutcome, AiHookError> {
        let path = self.settings_path(scope)?;
        let mut root = super::io::read_or_default_object(&path)?;
        let hooks = root
            .entry("hooks")
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(hooks_obj) = hooks else {
            return Err(AiHookError::InvalidEntry {
                name: "hooks".to_string(),
                reason: "existing `hooks` is not an object".to_string(),
            });
        };

        let mut warnings = Vec::new();
        let mut applied = 0usize;
        for entry in entries {
            let key = Self::event_key(entry.event)?;
            let arr = hooks_obj
                .entry(key.to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            let Value::Array(arr) = arr else {
                warnings.push(format!("{key} is not an array; skipping {}", entry.name));
                continue;
            };
            retain_non_jarvy_named(arr, &entry.name);
            arr.push(Self::entry_to_json(entry));
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
        let hooks = match root.get("hooks") {
            Some(Value::Object(m)) => m,
            _ => {
                outcome.missing = entries.iter().map(|e| e.name.clone()).collect();
                return Ok(outcome);
            }
        };

        let mut on_disk = Vec::new();
        for (_event, arr) in hooks {
            if let Value::Array(arr) = arr {
                on_disk.extend(collect_marker_names(arr));
            }
        }
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
        if let Some(Value::Object(hooks_obj)) = root.get_mut("hooks") {
            for (_event, arr) in hooks_obj.iter_mut() {
                if let Value::Array(arr) = arr {
                    let before = arr.len();
                    arr.retain(|v| v.get(JSON_MARKER_KEY).is_none());
                    removed += before - arr.len();
                }
            }
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
    fn pre_compact_is_unsupported() {
        let err = WindsurfProvisioner::event_key(HookEvent::PreCompact).unwrap_err();
        assert!(matches!(
            err,
            AiHookError::UnsupportedEvent {
                agent: "windsurf",
                ..
            }
        ));
    }

    #[test]
    fn entry_ships_both_command_and_powershell() {
        let entry = ResolvedEntry {
            name: "x".to_string(),
            library_source: None,
            event: HookEvent::PreToolUse,
            matcher: None,
            bash_command: Cow::Borrowed("exit 0"),
            windows_command: Cow::Borrowed("exit 0"),
            windows_warned: false,
            timeout_ms: 5_000,
        };
        let v = WindsurfProvisioner::entry_to_json(&entry);
        assert!(v.get("command").is_some());
        assert!(v.get("powershell").is_some());
        assert_eq!(v[JSON_HASH_KEY].as_str().unwrap().len(), 64);
    }
}
