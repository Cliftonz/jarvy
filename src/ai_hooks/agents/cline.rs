//! Cline provisioner (macOS / Linux only).
//!
//! Cline doesn't store hooks in JSON — it loads executable scripts from a
//! directory and dispatches by filename. We write a single dispatcher
//! script per touched event and one non-executable fragment per hook.
//! The dispatcher iterates the fragments and propagates the first
//! non-zero exit code.
//!
//! - User: `~/Documents/Cline/Rules/Hooks/`
//! - Project: `.clinerules/hooks/`
//!
//! Fragment naming: `<Event>.jarvy.<hook-name>.sh`. Cline doesn't
//! recognize the suffix — Jarvy's dispatcher loads them explicitly.

use std::collections::HashSet;
use std::path::PathBuf;

use super::io::home_or_err;
use super::markers::FILENAME_INFIX;
use super::{AgentProvisioner, ApplyOutcome, CheckOutcome, RemoveOutcome, ResolvedEntry};
use crate::ai_hooks::config::HookScope;
use crate::ai_hooks::error::AiHookError;
use crate::ai_hooks::event::HookEvent;

pub struct ClineProvisioner;

impl ClineProvisioner {
    const SLUG: &'static str = "cline";

    fn event_key(event: HookEvent) -> Result<&'static str, AiHookError> {
        Ok(match event {
            HookEvent::PreToolUse | HookEvent::PreShellExecution => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::SessionStart => "TaskStart",
            HookEvent::Stop => "TaskComplete",
            HookEvent::PreCompact => {
                return Err(AiHookError::UnsupportedEvent {
                    agent: "cline",
                    event: event.to_string(),
                });
            }
        })
    }

    fn hooks_dir(scope: HookScope) -> Result<PathBuf, AiHookError> {
        match scope {
            HookScope::User => Ok(home_or_err()?
                .join("Documents")
                .join("Cline")
                .join("Rules")
                .join("Hooks")),
            HookScope::Project => Ok(PathBuf::from(".clinerules").join("hooks")),
        }
    }

    fn fragment_name(event_key: &str, hook_name: &str) -> String {
        format!("{event_key}{FILENAME_INFIX}{hook_name}.sh")
    }

    fn dispatcher_body(event_key: &str) -> String {
        format!(
            "#!/usr/bin/env bash\n\
             # jarvy-managed dispatcher for {event_key}. Do not edit.\n\
             # Runs every jarvy fragment script in alphabetical order; any\n\
             # fragment that exits non-zero short-circuits the chain.\n\
             set -u\n\
             dir=\"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")\" && pwd)\"\n\
             status=0\n\
             for frag in \"$dir\"/{event_key}.jarvy.*.sh; do\n\
             \t[ -e \"$frag\" ] || continue\n\
             \tbash \"$frag\"\n\
             \trc=$?\n\
             \tif [ $rc -ne 0 ]; then status=$rc; break; fi\n\
             done\n\
             exit $status\n"
        )
    }

    fn fragment_body(entry: &ResolvedEntry<'_>) -> String {
        format!(
            "#!/usr/bin/env bash\n# jarvy-managed: {name}\n{body}",
            name = entry.name,
            body = entry.bash_command.as_ref(),
        )
    }

    /// Parse a fragment filename into its hook name. Avoids the per-file
    /// `OsString::into_string()` allocation by working over `&str` once.
    fn parse_fragment_name(name: &str) -> Option<(&str, &str)> {
        let stem = name.strip_suffix(".sh")?;
        let (event, rest) = stem.split_once(FILENAME_INFIX)?;
        Some((event, rest))
    }
}

impl AgentProvisioner for ClineProvisioner {
    fn slug(&self) -> &'static str {
        Self::SLUG
    }

    fn settings_path(&self, scope: HookScope) -> Result<PathBuf, AiHookError> {
        Self::hooks_dir(scope)
    }

    fn apply(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<ApplyOutcome, AiHookError> {
        if cfg!(target_os = "windows") {
            return Err(AiHookError::UnsupportedPlatform("cline"));
        }
        let dir = Self::hooks_dir(scope)?;
        std::fs::create_dir_all(&dir).map_err(|e| AiHookError::io(dir.clone(), e))?;

        let mut warnings = Vec::new();
        let mut applied = 0usize;
        let mut events_touched: std::collections::BTreeSet<&'static str> = Default::default();

        for entry in entries {
            let event_key = Self::event_key(entry.event)?;
            events_touched.insert(event_key);
            let frag_path = dir.join(Self::fragment_name(event_key, &entry.name));
            super::io::write_executable(&frag_path, &Self::fragment_body(entry))?;
            // Fragments are sourced by the dispatcher, not invoked
            // directly — drop the exec bit so Cline doesn't try to run
            // them itself.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&frag_path, std::fs::Permissions::from_mode(0o644));
            }
            if entry.windows_warned {
                warnings.push(format!(
                    "{}: Cline does not run on Windows — fragment ignored",
                    entry.name
                ));
            }
            applied += 1;
        }

        for event_key in &events_touched {
            let dispatch_path = dir.join(event_key);
            super::io::write_executable(&dispatch_path, &Self::dispatcher_body(event_key))?;
        }

        Ok(ApplyOutcome {
            agent: Self::SLUG,
            path: dir,
            applied,
            warnings,
        })
    }

    fn check(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<CheckOutcome, AiHookError> {
        let dir = Self::hooks_dir(scope)?;
        let mut outcome = CheckOutcome {
            agent: Self::SLUG,
            path: dir.clone(),
            ..CheckOutcome::default()
        };
        if !dir.exists() {
            outcome.missing = entries.iter().map(|e| e.name.clone()).collect();
            return Ok(outcome);
        }
        let on_disk: HashSet<String> = std::fs::read_dir(&dir)
            .map_err(|e| AiHookError::io(dir.clone(), e))?
            .filter_map(|r| r.ok())
            .filter_map(|d| {
                let name = d.file_name();
                let s = name.to_str()?;
                Self::parse_fragment_name(s).map(|(_, hook)| hook.to_string())
            })
            .collect();
        let desired: HashSet<String> = entries.iter().map(|e| e.name.clone()).collect();
        outcome.missing = desired.difference(&on_disk).cloned().collect();
        outcome.extra_jarvy = on_disk.difference(&desired).cloned().collect();
        outcome.missing.sort();
        outcome.extra_jarvy.sort();
        Ok(outcome)
    }

    fn remove(&self, scope: HookScope) -> Result<RemoveOutcome, AiHookError> {
        let dir = Self::hooks_dir(scope)?;
        if !dir.exists() {
            return Ok(RemoveOutcome {
                agent: Self::SLUG,
                path: dir,
                removed: 0,
                foreign_preserved: 0,
                warnings: Vec::new(),
            });
        }
        let mut removed = 0usize;
        // First pass: read every directory entry once, classify into
        // (fragment, event) or (other). Stops the previous N+1 scan
        // pattern.
        let mut fragments: Vec<(PathBuf, String)> = Vec::new();
        let mut other_entries: Vec<String> = Vec::new();
        for ent in std::fs::read_dir(&dir).map_err(|e| AiHookError::io(dir.clone(), e))? {
            let Ok(ent) = ent else {
                continue;
            };
            let path = ent.path();
            let Some(name) = ent.file_name().to_str().map(|s| s.to_string()) else {
                continue;
            };
            if let Some((event, _hook)) = Self::parse_fragment_name(&name) {
                fragments.push((path, event.to_string()));
            } else {
                other_entries.push(name);
            }
        }

        // Strip every fragment Jarvy owns.
        let mut events_touched: HashSet<String> = HashSet::new();
        for (path, event) in &fragments {
            let _ = std::fs::remove_file(path);
            events_touched.insert(event.clone());
            removed += 1;
        }

        // Drop the dispatcher when no fragments remain for that event.
        // After the strip above, no fragments remain for any touched
        // event, so unconditionally remove the dispatcher.
        for event_key in events_touched {
            if other_entries.contains(&event_key) {
                let _ = std::fs::remove_file(dir.join(&event_key));
            }
        }

        Ok(RemoveOutcome {
            agent: Self::SLUG,
            path: dir,
            removed,
            foreign_preserved: 0,
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_compact_unsupported() {
        let err = ClineProvisioner::event_key(HookEvent::PreCompact).unwrap_err();
        assert!(matches!(
            err,
            AiHookError::UnsupportedEvent { agent: "cline", .. }
        ));
    }

    #[test]
    fn fragment_filename_uses_event_and_name() {
        let n = ClineProvisioner::fragment_name("PreToolUse", "block-rm-rf");
        assert_eq!(n, "PreToolUse.jarvy.block-rm-rf.sh");
    }

    #[test]
    fn parse_fragment_name_roundtrip() {
        let name = ClineProvisioner::fragment_name("PreToolUse", "block-rm-rf");
        let (event, hook) = ClineProvisioner::parse_fragment_name(&name).unwrap();
        assert_eq!(event, "PreToolUse");
        assert_eq!(hook, "block-rm-rf");
    }

    #[test]
    fn parse_fragment_name_ignores_non_fragment() {
        assert!(ClineProvisioner::parse_fragment_name("PreToolUse").is_none());
        assert!(ClineProvisioner::parse_fragment_name("PreToolUse.jarvy.foo").is_none());
    }

    #[test]
    fn stop_maps_to_task_complete() {
        // Pin the chosen semantic mapping. HookEvent::Stop = agent
        // finishes normally → Cline's TaskComplete. (The previous
        // mapping to TaskCancel was a bug — cancel fires only on
        // user-initiated abort, missing the completion path.)
        assert_eq!(
            ClineProvisioner::event_key(HookEvent::Stop).unwrap(),
            "TaskComplete"
        );
    }

    #[test]
    fn dispatcher_loops_over_fragments() {
        let body = ClineProvisioner::dispatcher_body("PreToolUse");
        assert!(body.contains("PreToolUse.jarvy.*.sh"));
        assert!(body.contains("exit $status"));
    }
}
