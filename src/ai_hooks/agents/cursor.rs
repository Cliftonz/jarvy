//! Cursor provisioner.
//!
//! Writes `~/.cursor/hooks.json` (user) or `.cursor/hooks.json` (project).
//! Cursor's decision protocol differs from Claude Code: hooks emit
//! `{"permission":"deny"|"allow"|"ask"}` on stdout, with exit code 2 as
//! the cross-agent fallback deny (Cursor's default is fail-open; library
//! scripts exit 2 which Cursor honors).
//!
//! Wrapping policy: on Unix we ship `bash -c '<script>'`, on Windows we
//! use the `EncodedCommand` PowerShell wrapper from `platform.rs` so the
//! shell doesn't see any of the script's metacharacters.

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

pub struct CursorProvisioner;

impl CursorProvisioner {
    const SLUG: &'static str = "cursor";

    fn event_key(event: HookEvent) -> &'static str {
        match event {
            HookEvent::PreToolUse => "preToolUse",
            HookEvent::PostToolUse => "postToolUse",
            HookEvent::PreShellExecution => "beforeShellExecution",
            HookEvent::UserPromptSubmit => "beforeSubmitPrompt",
            HookEvent::SessionStart => "sessionStart",
            HookEvent::Stop => "stop",
            HookEvent::PreCompact => "preCompact",
        }
    }

    fn shim_unix(entry: &ResolvedEntry<'_>) -> String {
        // bash -c '<script>' with single-quote escaping. ' inside the
        // script is escaped as '\'' which closes the quote, types a
        // literal ', and reopens — POSIX-portable.
        let escaped = entry.bash_command.as_ref().replace('\'', "'\\''");
        format!("bash -c '{escaped}'")
    }

    fn entry_to_json(entry: &ResolvedEntry<'_>) -> Value {
        let command = match HookHost::current() {
            HookHost::Windows => wrap_powershell_command(entry.windows_command.as_ref()),
            HookHost::Unix => Self::shim_unix(entry),
        };
        let hash = entry_hash(entry);
        let mut obj = Map::new();
        obj.insert(
            JSON_MARKER_KEY.to_string(),
            Value::String(entry.name.clone()),
        );
        obj.insert(JSON_HASH_KEY.to_string(), Value::String(hash));
        obj.insert("command".to_string(), Value::String(command));
        if let Some(ref m) = entry.matcher {
            obj.insert("tool".to_string(), Value::String(m.clone()));
        }
        obj.insert(
            "timeout_ms".to_string(),
            Value::Number(entry.timeout_ms.into()),
        );
        Value::Object(obj)
    }
}

impl AgentProvisioner for CursorProvisioner {
    fn slug(&self) -> &'static str {
        Self::SLUG
    }

    fn settings_path(&self, scope: HookScope) -> Result<PathBuf, AiHookError> {
        match scope {
            HookScope::User => Ok(home_or_err()?.join(".cursor").join("hooks.json")),
            HookScope::Project => Ok(PathBuf::from(".cursor").join("hooks.json")),
        }
    }

    fn apply(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<ApplyOutcome, AiHookError> {
        let path = self.settings_path(scope)?;
        let mut root = super::io::read_or_default_object(&path)?;
        root.entry("version".to_string())
            .or_insert_with(|| json!(1));
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
            let key = Self::event_key(entry.event);
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

    fn sample_entry(name: &str) -> ResolvedEntry<'static> {
        ResolvedEntry {
            name: name.to_string(),
            library_source: Some(name.to_string()),
            event: HookEvent::PreToolUse,
            matcher: Some("Bash".to_string()),
            bash_command: Cow::Borrowed("echo 'hi'\n"),
            windows_command: Cow::Borrowed("Write-Host 'hi'\n"),
            windows_warned: false,
            timeout_ms: 5_000,
        }
    }

    #[test]
    fn shim_escapes_single_quotes() {
        let entry = sample_entry("x");
        let shim = CursorProvisioner::shim_unix(&entry);
        assert!(shim.starts_with("bash -c '"));
        assert!(shim.contains("'\\''"));
    }

    #[test]
    fn event_mapping_uses_camel_case() {
        assert_eq!(
            CursorProvisioner::event_key(HookEvent::PreShellExecution),
            "beforeShellExecution"
        );
    }

    #[test]
    fn entry_to_json_carries_marker_and_hash() {
        let entry = sample_entry("block-rm-rf");
        let v = CursorProvisioner::entry_to_json(&entry);
        assert_eq!(v[JSON_MARKER_KEY], "block-rm-rf");
        assert_eq!(v[JSON_HASH_KEY].as_str().unwrap().len(), 64);
    }
}
