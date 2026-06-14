//! Continue.dev provisioner.
//!
//! Continue has no executable-hook system — only a declarative
//! allow/ask/exclude policy in YAML. We translate library hooks whose
//! intent fits a glob deny (e.g. `block-rm-rf` → `exclude: ["Bash(rm
//! -rf*)"]`) and warn for everything else.
//!
//! Uses [`super::io::write_text_atomic`] for crash-safe writes, matching
//! the JSON provisioners.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::io::home_or_err;
use super::markers::{YAML_BLOCK_BEGIN, YAML_BLOCK_END};
use super::{AgentProvisioner, ApplyOutcome, CheckOutcome, RemoveOutcome, ResolvedEntry};
use crate::ai_hooks::config::HookScope;
use crate::ai_hooks::error::AiHookError;

pub struct ContinueProvisioner;

impl ContinueProvisioner {
    const SLUG: &'static str = "continue";

    /// Map well-known library hooks onto Continue glob deny patterns.
    /// Custom commands and hooks without a clear glob mapping are skipped
    /// with a warning.
    fn entry_to_globs(entry: &ResolvedEntry<'_>) -> Vec<&'static str> {
        let key = entry
            .library_source
            .as_deref()
            .unwrap_or(entry.name.as_str());
        match key {
            "block-rm-rf" => vec!["Bash(rm -rf*)", "Bash(rm -fr*)", "Bash(sudo rm*)"],
            "block-force-push" => vec!["Bash(git push * --force*)", "Bash(git push * -f*)"],
            "block-curl-bash-pipe" => vec!["Bash(curl *|*sh*)", "Bash(wget *|*sh*)"],
            "block-prod-db-write" => vec![
                "Bash(psql *prod*)",
                "Bash(mysql *prod*)",
                "Bash(*production*)",
            ],
            "block-git-reset-hard" => vec!["Bash(git reset --hard*)"],
            "block-protected-branch-commit" => vec![
                "Bash(git push origin main*)",
                "Bash(git push origin master*)",
            ],
            "block-kubectl-delete" => vec!["Bash(kubectl delete*)"],
            "block-docker-prune" => {
                vec!["Bash(docker system prune*)", "Bash(docker volume prune*)"]
            }
            "block-cat-env-files" => vec!["Bash(cat *.env*)", "Bash(printenv*)"],
            "block-malware-install" => vec![],
            "block-secrets-commit" => vec![],
            "block-edit-env-files" => vec![],
            "block-read-secret-files" => vec![],
            "block-drop-table" => vec!["Bash(*DROP TABLE*)", "Bash(*TRUNCATE TABLE*)"],
            "audit-log" => vec![],
            "commit-message-format-guard" => vec![],
            _ => vec![],
        }
    }

    /// Read the on-disk glob list inside the `jarvy-managed` block, used
    /// by `check` to compute `extra_jarvy` symmetrically with the other
    /// agents.
    fn parse_managed_globs(body: &str) -> BTreeSet<String> {
        let block = jarvy_block_body(body);
        let mut out = BTreeSet::new();
        for line in block.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("- ") else {
                continue;
            };
            let stripped = rest.trim().trim_matches('"').trim_matches('\'').to_string();
            if !stripped.is_empty() {
                out.insert(stripped);
            }
        }
        out
    }
}

impl AgentProvisioner for ContinueProvisioner {
    fn slug(&self) -> &'static str {
        Self::SLUG
    }

    fn settings_path(&self, scope: HookScope) -> Result<PathBuf, AiHookError> {
        match scope {
            HookScope::User => Ok(home_or_err()?.join(".continue").join("permissions.yaml")),
            HookScope::Project => Ok(PathBuf::from(".continue").join("permissions.yaml")),
        }
    }

    fn apply(
        &self,
        entries: &[ResolvedEntry<'_>],
        scope: HookScope,
    ) -> Result<ApplyOutcome, AiHookError> {
        let path = self.settings_path(scope)?;
        let mut warnings = Vec::new();
        let mut applied = 0usize;
        let mut excludes: BTreeSet<String> = BTreeSet::new();

        for entry in entries {
            let globs = Self::entry_to_globs(entry);
            if globs.is_empty() {
                warnings.push(format!(
                    "{}: no glob mapping for Continue — skipping (Continue is declarative-only)",
                    entry.name
                ));
                continue;
            }
            for g in globs {
                excludes.insert(g.to_string());
            }
            applied += 1;
        }

        let prior = std::fs::read_to_string(&path).unwrap_or_default();
        let stripped = strip_jarvy_block(&prior);
        let mut next = stripped.trim_end().to_string();
        if !next.is_empty() {
            next.push_str("\n\n");
        }
        next.push_str(YAML_BLOCK_BEGIN);
        next.push('\n');
        if excludes.is_empty() {
            next.push_str("# (no library hooks mapped to Continue glob patterns)\n");
        } else {
            next.push_str("exclude:\n");
            for g in &excludes {
                next.push_str("  - ");
                next.push_str(&yaml_quote(g));
                next.push('\n');
            }
        }
        next.push_str(YAML_BLOCK_END);
        next.push('\n');

        super::io::write_text_atomic(&path, &next)?;

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
        let mut outcome = CheckOutcome {
            agent: Self::SLUG,
            path: path.clone(),
            ..CheckOutcome::default()
        };
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        let on_disk_globs = Self::parse_managed_globs(&body);
        let mut desired_globs: BTreeSet<String> = BTreeSet::new();
        for entry in entries {
            for g in Self::entry_to_globs(entry) {
                desired_globs.insert(g.to_string());
            }
        }
        outcome.missing = desired_globs.difference(&on_disk_globs).cloned().collect();
        outcome.extra_jarvy = on_disk_globs.difference(&desired_globs).cloned().collect();
        outcome.missing.sort();
        outcome.extra_jarvy.sort();
        Ok(outcome)
    }

    fn remove(&self, scope: HookScope) -> Result<RemoveOutcome, AiHookError> {
        let path = self.settings_path(scope)?;
        let prior = std::fs::read_to_string(&path).unwrap_or_default();
        let stripped = strip_jarvy_block(&prior);
        let removed_count = Self::parse_managed_globs(&prior).len();
        if path.exists() {
            super::io::write_text_atomic(&path, &stripped)?;
        }
        Ok(RemoveOutcome {
            agent: Self::SLUG,
            path,
            removed: removed_count,
            foreign_preserved: 0,
            warnings: Vec::new(),
        })
    }
}

fn strip_jarvy_block(input: &str) -> String {
    let Some(start) = input.find(YAML_BLOCK_BEGIN) else {
        return input.to_string();
    };
    let Some(end_rel) = input[start..].find(YAML_BLOCK_END) else {
        return input.to_string();
    };
    let end = start + end_rel + YAML_BLOCK_END.len();
    let mut out = String::with_capacity(input.len());
    out.push_str(input[..start].trim_end());
    let tail = &input[end..];
    let tail_trimmed = tail.strip_prefix('\n').unwrap_or(tail);
    if !tail_trimmed.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(tail_trimmed);
    } else if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn jarvy_block_body(input: &str) -> &str {
    let Some(start) = input.find(YAML_BLOCK_BEGIN) else {
        return "";
    };
    let after = &input[start + YAML_BLOCK_BEGIN.len()..];
    let Some(end_rel) = after.find(YAML_BLOCK_END) else {
        return "";
    };
    &after[..end_rel]
}

fn yaml_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_hooks::event::HookEvent;
    use std::borrow::Cow;

    fn entry(name: &str, lib: Option<&str>) -> ResolvedEntry<'static> {
        ResolvedEntry {
            name: name.to_string(),
            library_source: lib.map(String::from),
            event: HookEvent::PreToolUse,
            matcher: Some("Bash".to_string()),
            bash_command: Cow::Borrowed(""),
            windows_command: Cow::Borrowed(""),
            windows_warned: false,
            timeout_ms: 5_000,
        }
    }

    #[test]
    fn rm_rf_maps_to_multiple_globs() {
        let g = ContinueProvisioner::entry_to_globs(&entry("block-rm-rf", Some("block-rm-rf")));
        assert!(g.iter().any(|s| s.contains("rm -rf")));
        assert!(g.iter().any(|s| s.contains("sudo rm")));
    }

    #[test]
    fn unknown_hook_yields_no_globs() {
        let g = ContinueProvisioner::entry_to_globs(&entry("custom-thing", None));
        assert!(g.is_empty());
    }

    #[test]
    fn strip_round_trip_preserves_user_block() {
        let input = "allow:\n  - Bash(ls*)\n\n# jarvy-managed begin\nexclude:\n  - \"x\"\n# jarvy-managed end\n";
        let stripped = strip_jarvy_block(input);
        assert!(stripped.contains("Bash(ls*)"));
        assert!(!stripped.contains("jarvy-managed"));
    }

    #[test]
    fn parse_managed_globs_extracts_yaml_list_items() {
        let body = "stuff\n# jarvy-managed begin\nexclude:\n  - \"Bash(rm -rf*)\"\n  - \"Bash(sudo rm*)\"\n# jarvy-managed end\n";
        let globs = ContinueProvisioner::parse_managed_globs(body);
        assert!(globs.contains("Bash(rm -rf*)"));
        assert!(globs.contains("Bash(sudo rm*)"));
    }
}
