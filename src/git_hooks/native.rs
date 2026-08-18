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

use super::config::{HookSource, NativeConfig};
use super::{HookError, HookInfo};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

    /// Resolve the effective hook bodies from ALL three sources:
    /// scanned `dir` + explicit `hooks` (inline OR file reference).
    /// Explicit entries win over dir-scanned ones for the same stage.
    ///
    /// Returns `Err` if any file path is unsafe or unreadable, or if
    /// an explicit `hooks` key isn't a known git hook stage.
    ///
    /// This is the single choke point for "where does the hook body
    /// come from" — install / run / list all consult it so their key
    /// sets agree.
    fn resolve_bodies(&self) -> Result<BTreeMap<String, String>, HookError> {
        let mut resolved: BTreeMap<String, String> = BTreeMap::new();

        // 1. Scan `dir` first — the explicit hooks map may override.
        if let Some(dir_rel) = &self.config.dir {
            let dir_abs = resolve_project_path(&self.project_dir, dir_rel)?;
            if !dir_abs.is_dir() {
                return Err(HookError::Config(format!(
                    "[git_hooks.native] dir `{}` is not a directory",
                    dir_abs.display()
                )));
            }
            for entry in std::fs::read_dir(&dir_abs).map_err(HookError::Io)? {
                let entry = entry.map_err(HookError::Io)?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if !is_stage_name(name) {
                    // Ignore README, .gitkeep, misspelled hook names.
                    // Not an error — dir-scan is intentionally lenient
                    // so a hooks folder can host adjacent files.
                    continue;
                }
                let body = std::fs::read_to_string(&path).map_err(HookError::Io)?;
                resolved.insert(name.to_string(), body);
            }
        }

        // 2. Explicit hooks map — validates stage name, overrides dir.
        for (stage, source) in &self.config.hooks {
            if !is_stage_name(stage) {
                return Err(HookError::Config(format!(
                    "[git_hooks.native.hooks] key '{stage}' is not a known git hook stage"
                )));
            }
            let body = match source {
                HookSource::Inline(s) => s.clone(),
                HookSource::File { file } => {
                    let abs = resolve_project_path(&self.project_dir, file)?;
                    std::fs::read_to_string(&abs).map_err(|e| {
                        HookError::Config(format!(
                            "[git_hooks.native.hooks.{stage}] file `{}` unreadable: {e}",
                            abs.display()
                        ))
                    })?
                }
            };
            resolved.insert(stage.clone(), body);
        }

        Ok(resolved)
    }

    pub fn install(&self) -> Result<(), HookError> {
        let hooks_dir = self.project_dir.join(".git").join("hooks");
        if !hooks_dir.is_dir() {
            return Err(HookError::NotAGitRepo);
        }

        let resolved = self.resolve_bodies()?;

        for (stage, body) in &resolved {
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
                count = resolved.len() as u64,
                scanned_dir = self.config.dir.is_some(),
                explicit_count = self.config.hooks.len() as u64,
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
    /// every configured hook if none is supplied. `all_files` is
    /// accepted for API parity but ignored — native hooks decide what
    /// to scan on their own.
    pub fn run(&self, _all_files: bool, hook_id: Option<&str>) -> Result<(), HookError> {
        let resolved = self.resolve_bodies()?;
        let hooks_dir = self.project_dir.join(".git").join("hooks");
        let to_run: Vec<&String> = match hook_id {
            Some(id) => {
                if !resolved.contains_key(id) {
                    return Err(HookError::RunFailed(format!(
                        "no native hook named `{id}` declared (checked [git_hooks.native.hooks] and dir scan)"
                    )));
                }
                vec![resolved.keys().find(|k| *k == id).unwrap()]
            }
            None => resolved.keys().collect(),
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
        // list must include dir-scanned hooks + explicit overrides so
        // `jarvy hooks list` matches what install would actually write.
        // Errors on unsafe file references bubble up rather than lie.
        let resolved = self.resolve_bodies()?;
        let mut out: Vec<HookInfo> = resolved
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

/// Resolve a config-supplied path (relative or absolute) against the
/// project root, refusing anything that would escape the repo.
///
/// Refusals: absolute paths, `..` components (even after joining),
/// canonical path outside `project_dir`. Symlinks are resolved by
/// `canonicalize()`, so a symlink under `scripts/hooks/pre-push` that
/// targets `/etc/passwd` fails the containment check.
fn resolve_project_path(project_dir: &Path, rel: &str) -> Result<PathBuf, HookError> {
    let candidate = Path::new(rel);
    if candidate.is_absolute() {
        return Err(HookError::Config(format!(
            "[git_hooks.native] path `{rel}` is absolute; use a repo-relative path"
        )));
    }
    if candidate
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(HookError::Config(format!(
            "[git_hooks.native] path `{rel}` contains `..`; hooks must live inside the project"
        )));
    }
    let joined = project_dir.join(candidate);
    // Canonicalize when the path exists so symlink traversal is
    // detected. If it doesn't exist yet, fall back to a lexical
    // containment check against the joined path.
    let (canonical, base) = match (joined.canonicalize(), project_dir.canonicalize()) {
        (Ok(c), Ok(b)) => (c, b),
        _ => (joined.clone(), project_dir.to_path_buf()),
    };
    if !canonical.starts_with(&base) {
        return Err(HookError::Config(format!(
            "[git_hooks.native] path `{rel}` resolves outside the project root"
        )));
    }
    Ok(joined)
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
            dir: None,
            hooks: hooks
                .into_iter()
                .map(|(k, v)| (k.to_string(), HookSource::inline(v)))
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

    #[test]
    fn install_reads_file_reference_from_disk() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        fs::create_dir_all(tmp.path().join("scripts/hooks")).unwrap();
        fs::write(
            tmp.path().join("scripts/hooks/pre-push.sh"),
            "#!/bin/sh\necho pushing\n",
        )
        .unwrap();

        let mut hooks = BTreeMap::new();
        hooks.insert(
            "pre-push".to_string(),
            HookSource::File {
                file: "scripts/hooks/pre-push.sh".to_string(),
            },
        );
        let cfg = NativeConfig { dir: None, hooks };
        NativeHandler::new(cfg, tmp.path().to_path_buf())
            .install()
            .unwrap();

        let content = fs::read_to_string(tmp.path().join(".git/hooks/pre-push")).unwrap();
        assert!(content.starts_with("#!/bin/sh\n"));
        assert!(content.contains(JARVY_MARKER));
        assert!(content.contains("echo pushing"));
    }

    #[test]
    fn install_scans_dir_for_stage_named_files() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let hooks_src = tmp.path().join("scripts/hooks");
        fs::create_dir_all(&hooks_src).unwrap();
        fs::write(hooks_src.join("pre-commit"), "cargo fmt --check\n").unwrap();
        fs::write(hooks_src.join("pre-push"), "cargo test\n").unwrap();
        // README + .gitkeep must be ignored, not error.
        fs::write(hooks_src.join("README.md"), "hooks live here").unwrap();
        fs::write(hooks_src.join(".gitkeep"), "").unwrap();

        let cfg = NativeConfig {
            dir: Some("scripts/hooks".to_string()),
            hooks: BTreeMap::new(),
        };
        NativeHandler::new(cfg, tmp.path().to_path_buf())
            .install()
            .unwrap();

        assert!(tmp.path().join(".git/hooks/pre-commit").exists());
        assert!(tmp.path().join(".git/hooks/pre-push").exists());
        // README should NOT be installed as a hook.
        assert!(!tmp.path().join(".git/hooks/README.md").exists());
    }

    #[test]
    fn explicit_hook_overrides_dir_scanned_entry() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let hooks_src = tmp.path().join("scripts/hooks");
        fs::create_dir_all(&hooks_src).unwrap();
        fs::write(hooks_src.join("pre-commit"), "from-dir\n").unwrap();

        let mut hooks = BTreeMap::new();
        hooks.insert(
            "pre-commit".to_string(),
            HookSource::inline("from-explicit\n"),
        );
        let cfg = NativeConfig {
            dir: Some("scripts/hooks".to_string()),
            hooks,
        };
        NativeHandler::new(cfg, tmp.path().to_path_buf())
            .install()
            .unwrap();

        let content = fs::read_to_string(tmp.path().join(".git/hooks/pre-commit")).unwrap();
        assert!(
            content.contains("from-explicit"),
            "explicit hooks map must win over dir scan; got:\n{content}"
        );
        assert!(!content.contains("from-dir"));
    }

    #[test]
    fn file_reference_refuses_absolute_path() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let mut hooks = BTreeMap::new();
        hooks.insert(
            "pre-commit".to_string(),
            HookSource::File {
                file: "/etc/passwd".to_string(),
            },
        );
        let err = NativeHandler::new(NativeConfig { dir: None, hooks }, tmp.path().to_path_buf())
            .install()
            .expect_err("must refuse absolute path");
        assert!(matches!(err, HookError::Config(_)), "got {err:?}");
    }

    #[test]
    fn file_reference_refuses_parent_traversal() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let mut hooks = BTreeMap::new();
        hooks.insert(
            "pre-commit".to_string(),
            HookSource::File {
                file: "../../../etc/passwd".to_string(),
            },
        );
        let err = NativeHandler::new(NativeConfig { dir: None, hooks }, tmp.path().to_path_buf())
            .install()
            .expect_err("must refuse traversal");
        assert!(matches!(err, HookError::Config(_)), "got {err:?}");
    }

    #[test]
    fn dir_refuses_absolute_path() {
        let tmp = tempdir().unwrap();
        make_repo(tmp.path());
        let cfg = NativeConfig {
            dir: Some("/tmp/evil-hooks".to_string()),
            hooks: BTreeMap::new(),
        };
        let err = NativeHandler::new(cfg, tmp.path().to_path_buf())
            .install()
            .expect_err("must refuse absolute dir");
        assert!(matches!(err, HookError::Config(_)), "got {err:?}");
    }

    #[test]
    fn list_includes_dir_scanned_hooks() {
        let tmp = tempdir().unwrap();
        let hooks_src = tmp.path().join("scripts/hooks");
        fs::create_dir_all(&hooks_src).unwrap();
        fs::write(hooks_src.join("pre-commit"), "x").unwrap();
        fs::write(hooks_src.join("commit-msg"), "y").unwrap();

        let cfg = NativeConfig {
            dir: Some("scripts/hooks".to_string()),
            hooks: BTreeMap::new(),
        };
        let hooks = NativeHandler::new(cfg, tmp.path().to_path_buf())
            .list()
            .unwrap();
        let ids: Vec<&str> = hooks.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["commit-msg", "pre-commit"]);
    }
}
