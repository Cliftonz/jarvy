//! Claude Code provisioner.
//!
//! Writes `~/.claude/settings.json` (user) or `.claude/settings.json`
//! (project). Hooks shape:
//!
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "_jarvy_managed": "block-rm-rf",
//!         "_jarvy_sha256": "<hex>",
//!         "matcher": "Bash",
//!         "hooks": [
//!           { "type": "command", "command": "...", "timeout": 5 }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```

use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::io::home_or_err;
use super::json_merge::{collect_marker_names, entry_hash, retain_non_jarvy_named};
use super::markers::{JSON_HASH_KEY, JSON_MARKER_KEY};
use super::{AgentProvisioner, ApplyOutcome, CheckOutcome, RemoveOutcome, ResolvedEntry};
use crate::ai_hooks::config::HookScope;
use crate::ai_hooks::error::AiHookError;
use crate::ai_hooks::event::HookEvent;
use crate::ai_hooks::platform::{HookHost, wrap_powershell_command};

pub struct ClaudeCodeProvisioner;

impl ClaudeCodeProvisioner {
    const SLUG: &'static str = "claude-code";

    fn event_key(event: HookEvent) -> Result<&'static str, AiHookError> {
        Ok(match event {
            HookEvent::PreToolUse | HookEvent::PreShellExecution => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::SessionStart => "SessionStart",
            HookEvent::Stop => "Stop",
            HookEvent::PreCompact => "PreCompact",
        })
    }

    fn entry_to_json(entry: &ResolvedEntry<'_>) -> Value {
        // Pick the command for the current host. Cross-compiled-then-run
        // would still work as long as we ask at runtime (HookHost::current
        // wraps cfg!(target_os) in a function so future host-detection
        // refinements live in one place).
        let command = match HookHost::current() {
            HookHost::Windows => wrap_powershell_command(entry.windows_command.as_ref()),
            HookHost::Unix => entry.bash_command.as_ref().to_string(),
        };
        let hash = entry_hash(entry);
        json!({
            JSON_MARKER_KEY: entry.name,
            JSON_HASH_KEY: hash,
            "matcher": entry.matcher.clone().unwrap_or_default(),
            "hooks": [
                {
                    "type": "command",
                    "command": command,
                    "timeout": entry.timeout_ms / 1000,
                }
            ]
        })
    }
}

impl AgentProvisioner for ClaudeCodeProvisioner {
    fn slug(&self) -> &'static str {
        Self::SLUG
    }

    fn settings_path(&self, scope: HookScope) -> Result<PathBuf, AiHookError> {
        match scope {
            HookScope::User => Ok(home_or_err()?.join(".claude").join("settings.json")),
            HookScope::Project => Ok(PathBuf::from(".claude").join("settings.json")),
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
                reason: "existing `hooks` field is not an object".to_string(),
            });
        };

        let mut warnings = Vec::new();
        let mut applied = 0usize;
        let host_is_windows = matches!(HookHost::current(), HookHost::Windows);

        for entry in entries {
            let event_key = Self::event_key(entry.event)?;
            let list = hooks_obj
                .entry(event_key.to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            let Value::Array(list) = list else {
                warnings.push(format!(
                    "{event_key} entry is not an array; skipping {}",
                    entry.name
                ));
                continue;
            };
            retain_non_jarvy_named(list, &entry.name);
            list.push(Self::entry_to_json(entry));
            if entry.windows_warned && host_is_windows {
                warnings.push(format!(
                    "{}: Windows command auto-translated from bash — verify behavior",
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

        let hooks = match root.get("hooks") {
            Some(Value::Object(m)) => m,
            _ => {
                outcome.missing = entries.iter().map(|e| e.name.clone()).collect();
                return Ok(outcome);
            }
        };

        let mut on_disk: Vec<String> = Vec::new();
        for (_event, list) in hooks {
            if let Value::Array(arr) = list {
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

        // `remove` strips every entry carrying the marker key.
        // `apply`'s impersonation defense is the right place to refuse
        // foreign hash mismatches — at remove time the operator's intent
        // is "wipe everything Jarvy has ever owned here", including
        // legacy entries that pre-date the hash field.
        if let Some(Value::Object(hooks_obj)) = root.get_mut("hooks") {
            for (_event, list) in hooks_obj.iter_mut() {
                if let Value::Array(arr) = list {
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
    use crate::ai_hooks::event::HookEvent;
    use std::borrow::Cow;

    fn sample_entry(name: &str) -> ResolvedEntry<'static> {
        ResolvedEntry {
            name: name.to_string(),
            library_source: Some(name.to_string()),
            event: HookEvent::PreToolUse,
            matcher: Some("Bash".to_string()),
            bash_command: Cow::Borrowed("exit 0\n"),
            windows_command: Cow::Borrowed("exit 0\n"),
            windows_warned: false,
            timeout_ms: 5_000,
        }
    }

    #[test]
    fn entry_to_json_carries_marker_and_hash() {
        let entry = sample_entry("block-rm-rf");
        let v = ClaudeCodeProvisioner::entry_to_json(&entry);
        assert_eq!(v[JSON_MARKER_KEY], "block-rm-rf");
        assert!(v[JSON_HASH_KEY].as_str().unwrap().len() == 64);
    }

    #[test]
    fn event_mapping_uses_pre_tool_use_for_shell() {
        assert_eq!(
            ClaudeCodeProvisioner::event_key(HookEvent::PreShellExecution).unwrap(),
            "PreToolUse"
        );
    }
}
