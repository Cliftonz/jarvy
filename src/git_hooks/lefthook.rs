//! Lefthook framework handler.
//!
//! [lefthook](https://github.com/evilmartians/lefthook) is a single Go
//! binary with parallel hook execution. Config lives in `lefthook.yml`
//! at the repo root.
//!
//! Jarvy's lefthook handler:
//!
//! 1. Requires `lefthook` on PATH (surfaces as `FrameworkNotInstalled`).
//! 2. `install`: `lefthook install` (writes `.git/hooks/<name>` shims
//!    pointing at `lefthook run <name>`). Idempotent.
//! 3. `update`: `lefthook self-update`. Falls back to "user manages
//!    the binary" if `self-update` isn't supported (e.g. brew install).
//! 4. `run`: `lefthook run pre-commit` or whatever `hook_id` is given;
//!    `all_files = true` translates to `--all-files`.
//! 5. `list`: parses `lefthook.yml` and enumerates the commands under
//!    each hook stage.

use super::{HookError, HookInfo};
use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny, MapAccess, Visitor};
use std::fmt;
use std::path::PathBuf;
use std::process::Command;

pub struct LefthookHandler {
    project_dir: PathBuf,
}

impl LefthookHandler {
    pub fn new(project_dir: PathBuf) -> Self {
        Self { project_dir }
    }

    pub fn install(&self) -> Result<(), HookError> {
        if !is_installed() {
            return Err(HookError::FrameworkNotInstalled("lefthook".to_string()));
        }

        let config_path = self.project_dir.join("lefthook.yml");
        if !config_path.exists() {
            return Err(HookError::Config(format!(
                "lefthook config not found at {}; create lefthook.yml first",
                config_path.display()
            )));
        }

        let status = Command::new("lefthook")
            .arg("install")
            .current_dir(&self.project_dir)
            .status()
            .map_err(HookError::Io)?;
        if !status.success() {
            return Err(HookError::InstallFailed(format!(
                "lefthook install exited with {}",
                status.code().unwrap_or(-1)
            )));
        }

        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(event = "git_hooks.installed", framework = "lefthook");
        }
        Ok(())
    }

    pub fn update(&self) -> Result<(), HookError> {
        if !is_installed() {
            return Err(HookError::FrameworkNotInstalled("lefthook".to_string()));
        }
        // `lefthook self-update` exists in recent versions but isn't
        // available in every install method (brew users update via
        // `brew upgrade`). Treat a non-zero exit as advisory — we
        // still re-run `install` to refresh the git-hook shims.
        let _ = Command::new("lefthook")
            .arg("self-update")
            .current_dir(&self.project_dir)
            .status();

        let status = Command::new("lefthook")
            .arg("install")
            .current_dir(&self.project_dir)
            .status()
            .map_err(HookError::Io)?;
        if !status.success() {
            return Err(HookError::UpdateFailed(format!(
                "lefthook install (after self-update) exited with {}",
                status.code().unwrap_or(-1)
            )));
        }

        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(event = "git_hooks.updated", framework = "lefthook");
        }
        Ok(())
    }

    pub fn run(&self, all_files: bool, hook_id: Option<&str>) -> Result<(), HookError> {
        if !is_installed() {
            return Err(HookError::FrameworkNotInstalled("lefthook".to_string()));
        }
        // `lefthook run` requires a hook stage name — default to
        // pre-commit when none is supplied (mirrors `pre-commit run`'s
        // implicit default).
        let stage = hook_id.unwrap_or("pre-commit");
        let mut cmd = Command::new("lefthook");
        cmd.arg("run").arg(stage);
        if all_files {
            cmd.arg("--all-files");
        }
        cmd.current_dir(&self.project_dir);

        let status = cmd.status().map_err(HookError::Io)?;
        if !status.success() {
            return Err(HookError::RunFailed(format!(
                "lefthook run {} exited with {}",
                stage,
                status.code().unwrap_or(-1)
            )));
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<HookInfo>, HookError> {
        let config_path = self.project_dir.join("lefthook.yml");
        if !config_path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&config_path)?;
        // lefthook.yml mixes stages with top-level config keys
        // (`skip_output`, `extends`, ...). Only stage-shaped entries are
        // read; everything else is skipped without being materialized.
        let options = serde_saphyr::options! {
            non_finite_float_policy: serde_saphyr::NonFiniteFloatPolicy::PassThrough,
        };
        let parsed: std::collections::BTreeMap<String, LefthookEntry> =
            serde_saphyr::from_str_with_options(&content, options)
                .map_err(|e| HookError::Config(format!("parse {}: {e}", config_path.display())))?;

        let mut hooks = Vec::new();
        for (stage, entry) in &parsed {
            if !is_stage_name(stage) {
                continue;
            }
            let LefthookEntry::Stage { commands } = entry else {
                continue;
            };
            for id in &commands.0 {
                hooks.push(HookInfo {
                    id: id.clone(),
                    repo: "local".to_string(),
                    version: String::new(),
                    hook_type: stage.clone(),
                });
            }
        }
        hooks.sort_by(|a, b| {
            (a.hook_type.as_str(), a.id.as_str()).cmp(&(b.hook_type.as_str(), b.id.as_str()))
        });
        Ok(hooks)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum LefthookEntry {
    Stage { commands: CommandIds },
    Other(IgnoredAny),
}

struct CommandIds(Vec<String>);

#[derive(Deserialize)]
#[serde(untagged)]
enum CommandKey {
    Name(String),
    Other(IgnoredAny),
}

impl<'de> Deserialize<'de> for CommandIds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CommandIdsVisitor;

        impl<'de> Visitor<'de> for CommandIdsVisitor {
            type Value = CommandIds;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a commands mapping")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<CommandIds, A::Error> {
                let mut ids = Vec::new();
                while let Some(key) = map.next_key::<CommandKey>()? {
                    map.next_value::<IgnoredAny>()?;
                    if let CommandKey::Name(id) = key {
                        ids.push(id);
                    }
                }
                Ok(CommandIds(ids))
            }
        }

        deserializer.deserialize_map(CommandIdsVisitor)
    }
}

/// Known git hook stages. We refuse to surface non-hook top-level
/// keys (`skip_output`, `extends`, …) as `HookInfo` entries.
fn is_stage_name(s: &str) -> bool {
    matches!(
        s,
        "pre-commit"
            | "pre-push"
            | "pre-merge-commit"
            | "post-commit"
            | "post-checkout"
            | "post-merge"
            | "post-rewrite"
            | "commit-msg"
            | "prepare-commit-msg"
            | "applypatch-msg"
            | "pre-applypatch"
            | "post-applypatch"
            | "pre-rebase"
            | "pre-receive"
            | "update"
            | "post-receive"
            | "post-update"
            | "push-to-checkout"
            | "fsmonitor-watchman"
            | "p4-changelist"
            | "p4-prepare-changelist"
            | "p4-post-changelist"
            | "p4-pre-submit"
            | "sendemail-validate"
    )
}

fn is_installed() -> bool {
    Command::new("lefthook")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn list_returns_empty_when_no_config() {
        let tmp = tempdir().unwrap();
        let handler = LefthookHandler::new(tmp.path().to_path_buf());
        assert!(handler.list().unwrap().is_empty());
    }

    #[test]
    fn list_parses_stages_and_commands() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("lefthook.yml"),
            r#"
pre-commit:
  parallel: true
  commands:
    rustfmt:
      run: cargo fmt --check
    clippy:
      run: cargo clippy
commit-msg:
  commands:
    conventional:
      run: ./scripts/check-commit.sh {1}
skip_output:
  - meta
"#,
        )
        .unwrap();
        let handler = LefthookHandler::new(tmp.path().to_path_buf());
        let hooks = handler.list().unwrap();
        assert_eq!(hooks.len(), 3, "got {hooks:?}");
        // skip_output is NOT a stage and must be filtered out.
        assert!(hooks.iter().all(|h| h.hook_type != "skip_output"));
        assert!(
            hooks
                .iter()
                .any(|h| h.id == "rustfmt" && h.hook_type == "pre-commit")
        );
        assert!(
            hooks
                .iter()
                .any(|h| h.id == "conventional" && h.hook_type == "commit-msg")
        );
    }

    #[test]
    fn list_ignores_complex_keys_outside_stages() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("lefthook.yml"),
            r#"
pre-commit:
  commands:
    lint:
      run: cargo clippy
settings:
  ? [a, b]
  : c
"#,
        )
        .unwrap();
        let handler = LefthookHandler::new(tmp.path().to_path_buf());
        let hooks = handler.list().unwrap();
        assert_eq!(hooks.len(), 1, "got {hooks:?}");
        assert_eq!(hooks[0].id, "lint");
    }

    #[test]
    fn list_ignores_non_finite_floats_outside_stages() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("lefthook.yml"),
            r#"
pre-commit:
  commands:
    lint:
      run: cargo clippy
settings:
  ratio: .nan
"#,
        )
        .unwrap();
        let handler = LefthookHandler::new(tmp.path().to_path_buf());
        let hooks = handler.list().unwrap();
        assert_eq!(hooks.len(), 1, "got {hooks:?}");
        assert_eq!(hooks[0].id, "lint");
    }

    #[test]
    fn list_rejects_malformed_yaml() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("lefthook.yml"), "not: valid: yaml: at all:").unwrap();
        let handler = LefthookHandler::new(tmp.path().to_path_buf());
        let err = handler.list().expect_err("malformed yaml must error");
        assert!(matches!(err, HookError::Config(_)), "got {err:?}");
    }

    #[test]
    fn install_requires_config_file() {
        // Only meaningful when lefthook itself is installed.
        if !is_installed() {
            return;
        }
        let tmp = tempdir().unwrap();
        let handler = LefthookHandler::new(tmp.path().to_path_buf());
        let err = handler.install().expect_err("must error without config");
        assert!(matches!(err, HookError::Config(_)), "got {err:?}");
    }

    #[test]
    fn is_stage_name_table() {
        assert!(is_stage_name("pre-commit"));
        assert!(is_stage_name("commit-msg"));
        assert!(is_stage_name("pre-push"));
        assert!(!is_stage_name("skip_output"));
        assert!(!is_stage_name("extends"));
        assert!(!is_stage_name("random"));
    }
}
