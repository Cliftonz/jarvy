//! Native git hooks handler — no framework process between git and
//! your script. Hook bodies live in `[git_hooks.native.hooks]` keyed
//! by stage name; on `install` we write them straight into
//! `.git/hooks/<stage>` with a `#!/bin/sh` shebang and a stamped marker
//! comment.
//!
//! The marker comment (`# managed by jarvy — [git_hooks.native]`)
//! lets `install` recognize its own prior output and overwrite safely.
//! If the existing `.git/hooks/<stage>` has DIFFERENT content and
//! lacks the marker, install refuses with `HookError::InstallFailed`
//! so we never silently clobber a hand-rolled hook.
//!
//! No `update` path — Jarvy doesn't manage your hook bodies' versions.
//! Re-run `install` after editing `[git_hooks.native.hooks]`.

use super::config::NativeConfig;
use super::{HookError, HookInfo};
use std::path::PathBuf;

const JARVY_MARKER: &str = "# managed by jarvy — [git_hooks.native]";

pub struct NativeHandler {
    config: NativeConfig,
    project_dir: PathBuf,
}

impl NativeHandler {
    pub fn new(config: NativeConfig, project_dir: PathBuf) -> Self {
        Self {
            config,
            project_dir,
        }
    }

    pub fn install(&self) -> Result<(), HookError> {
        let hooks_dir = self.project_dir.join(".git").join("hooks");
        if !hooks_dir.is_dir() {
            return Err(HookError::NotAGitRepo);
        }

        for (stage, body) in &self.config.hooks {
            if !is_stage_name(stage) {
                return Err(HookError::Config(format!(
                    "[git_hooks.native.hooks] key '{stage}' is not a known git hook stage"
                )));
            }
            let target = hooks_dir.join(stage);
            if let Ok(existing) = std::fs::read_to_string(&target)
                && !existing.contains(JARVY_MARKER)
            {
                return Err(HookError::InstallFailed(format!(
                    "{} exists and was not written by jarvy; refusing to overwrite a hand-rolled hook (move it aside, or add the `{JARVY_MARKER}` marker manually)",
                    target.display()
                )));
            }

            // git requires `#!` on line 1. Either honor the user's
            // shebang and inject the marker on line 2, or insert
            // `#!/bin/sh` then the marker then the body.
            let trimmed = body.trim_start();
            let (shebang_line, rest) = if let Some(rest) = trimmed.strip_prefix("#!") {
                let nl = rest.find('\n').unwrap_or(rest.len());
                (format!("#!{}", &rest[..nl]), &rest[nl..])
            } else {
                ("#!/bin/sh".to_string(), trimmed)
            };
            let mut content = String::with_capacity(body.len() + 96);
            content.push_str(&shebang_line);
            content.push('\n');
            content.push_str(JARVY_MARKER);
            content.push('\n');
            // Skip a single leading newline on `rest` so we don't
            // emit a blank line between marker and body.
            let body_tail = rest.strip_prefix('\n').unwrap_or(rest);
            content.push_str(body_tail);
            if !content.ends_with('\n') {
                content.push('\n');
            }

            atomic_write_executable(&target, &content)?;
        }

        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(
                event = "git_hooks.installed",
                framework = "native",
                count = self.config.hooks.len() as u64,
            );
        }
        Ok(())
    }

    /// Native hooks have no upstream to autoupdate — re-running install
    /// is the only meaningful "update" operation.
    pub fn update(&self) -> Result<(), HookError> {
        self.install()?;
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(event = "git_hooks.updated", framework = "native");
        }
        Ok(())
    }

    /// Execute the configured native hook script for `hook_id`, or
    /// every hook in `[git_hooks.native.hooks]` if none is supplied.
    /// `all_files` is accepted for API parity but ignored — native
    /// hooks decide what to scan on their own.
    pub fn run(&self, _all_files: bool, hook_id: Option<&str>) -> Result<(), HookError> {
        let hooks_dir = self.project_dir.join(".git").join("hooks");
        let to_run: Vec<&String> = match hook_id {
            Some(id) => {
                if !self.config.hooks.contains_key(id) {
                    return Err(HookError::RunFailed(format!(
                        "no native hook named `{id}` declared in [git_hooks.native.hooks]"
                    )));
                }
                vec![self.config.hooks.keys().find(|k| *k == id).unwrap()]
            }
            None => self.config.hooks.keys().collect(),
        };

        if to_run.is_empty() {
            return Ok(());
        }

        let mut had_failure = false;
        for stage in to_run {
            let path = hooks_dir.join(stage);
            if !path.exists() {
                eprintln!("  native hook `{stage}` not installed; run `jarvy hooks install` first");
                had_failure = true;
                continue;
            }
            let status = std::process::Command::new("sh")
                .arg(&path)
                .current_dir(&self.project_dir)
                .status()
                .map_err(HookError::Io)?;
            if !status.success() {
                eprintln!(
                    "  native hook `{stage}` exited with {}",
                    status.code().unwrap_or(-1)
                );
                had_failure = true;
            }
        }
        if had_failure {
            return Err(HookError::RunFailed(
                "one or more native hooks failed".to_string(),
            ));
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<HookInfo>, HookError> {
        let mut out: Vec<HookInfo> = self
            .config
            .hooks
            .keys()
            .map(|stage| HookInfo {
                id: stage.clone(),
                repo: "local".to_string(),
                version: String::new(),
                hook_type: stage.clone(),
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }
}

fn is_stage_name(s: &str) -> bool {
    // Same list as the lefthook handler — keep in sync if either
    // adds support for a new git hook stage.
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

/// Atomic write + `chmod +x` so git can execute the script.
fn atomic_write_executable(target: &std::path::Path, content: &str) -> Result<(), HookError> {
    use std::io::Write;
    let parent = target.parent().unwrap_or(std::path::Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(HookError::Io)?;
    tmp.as_file_mut()
        .write_all(content.as_bytes())
        .map_err(HookError::Io)?;
    tmp.as_file_mut().flush().map_err(HookError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(tmp.path())
            .map_err(HookError::Io)?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(tmp.path(), perms).map_err(HookError::Io)?;
    }
    tmp.persist(target)
        .map_err(|e| HookError::Io(std::io::Error::other(format!("persist failed: {e}"))))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;

    fn cfg_with(hooks: Vec<(&str, &str)>) -> NativeConfig {
        NativeConfig {
            hooks: hooks
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    fn make_repo(tmp: &std::path::Path) {
        fs::create_dir_all(tmp.join(".git").join("hooks")).unwrap();
    }

    #[test]
    fn install_writes_hook_with_marker_and_shebang() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let handler = NativeHandler::new(
            cfg_with(vec![("pre-commit", "cargo fmt --check || exit 1\n")]),
            tmp.path().to_path_buf(),
        );
        handler.install().unwrap();
        let content = fs::read_to_string(tmp.path().join(".git/hooks/pre-commit")).unwrap();
        assert!(content.starts_with("#!/bin/sh\n"));
        assert!(content.contains(JARVY_MARKER));
        assert!(content.contains("cargo fmt --check"));
    }

    #[test]
    fn install_refuses_to_overwrite_unmanaged_hook() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let target = tmp.path().join(".git/hooks/pre-commit");
        fs::write(&target, "#!/bin/sh\necho 'user-authored hook'\n").unwrap();
        let handler = NativeHandler::new(
            cfg_with(vec![("pre-commit", "cargo fmt --check\n")]),
            tmp.path().to_path_buf(),
        );
        let err = handler.install().expect_err("must refuse");
        assert!(matches!(err, HookError::InstallFailed(_)), "got {err:?}");
        // File must be unchanged.
        let preserved = fs::read_to_string(&target).unwrap();
        assert!(preserved.contains("user-authored hook"));
    }

    #[test]
    fn install_overwrites_own_marked_output() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let handler = NativeHandler::new(
            cfg_with(vec![("pre-commit", "echo first\n")]),
            tmp.path().to_path_buf(),
        );
        handler.install().unwrap();
        // Second run with different body — should succeed because the
        // marker is present.
        let handler2 = NativeHandler::new(
            cfg_with(vec![("pre-commit", "echo second\n")]),
            tmp.path().to_path_buf(),
        );
        handler2.install().unwrap();
        let content = fs::read_to_string(tmp.path().join(".git/hooks/pre-commit")).unwrap();
        assert!(content.contains("echo second"));
        assert!(!content.contains("echo first"));
    }

    #[test]
    fn install_preserves_user_shebang() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let handler = NativeHandler::new(
            cfg_with(vec![("pre-commit", "#!/usr/bin/env bash\necho hi\n")]),
            tmp.path().to_path_buf(),
        );
        handler.install().unwrap();
        let content = fs::read_to_string(tmp.path().join(".git/hooks/pre-commit")).unwrap();
        assert!(
            content.starts_with("#!/usr/bin/env bash\n"),
            "got:\n{content}"
        );
    }

    #[test]
    fn install_refuses_unknown_stage_name() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let handler = NativeHandler::new(
            cfg_with(vec![("not-a-real-stage", "")]),
            tmp.path().to_path_buf(),
        );
        let err = handler.install().expect_err("must error");
        assert!(matches!(err, HookError::Config(_)), "got {err:?}");
    }

    #[test]
    fn list_returns_one_entry_per_configured_stage() {
        let tmp = tempdir().unwrap();
        let handler = NativeHandler::new(
            cfg_with(vec![("pre-commit", "x"), ("commit-msg", "y")]),
            tmp.path().to_path_buf(),
        );
        let hooks = handler.list().unwrap();
        let ids: Vec<&str> = hooks.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["commit-msg", "pre-commit"]);
    }

    #[test]
    fn install_errors_when_not_a_git_repo() {
        let tmp = tempdir().unwrap();
        // No .git dir.
        let handler = NativeHandler::new(
            cfg_with(vec![("pre-commit", "echo hi")]),
            tmp.path().to_path_buf(),
        );
        let err = handler.install().expect_err("must error");
        assert!(matches!(err, HookError::NotAGitRepo), "got {err:?}");
    }
}
