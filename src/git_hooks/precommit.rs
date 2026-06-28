//! `pre-commit` framework handler.
//!
//! Shells out to the `pre-commit` CLI for install / autoupdate / run.
//! Parses `.pre-commit-config.yaml` directly with `serde_yaml` for the
//! `list` command so we don't need to invoke the framework to discover
//! what hooks are configured.

use super::config::PreCommitConfig;
use super::{HookError, HookInfo};
use std::path::PathBuf;
use std::process::Command;

pub struct PreCommitHandler {
    config: PreCommitConfig,
    project_dir: PathBuf,
}

impl PreCommitHandler {
    pub fn new(config: PreCommitConfig, project_dir: PathBuf) -> Self {
        Self {
            config,
            project_dir,
        }
    }

    pub fn install(&self) -> Result<(), HookError> {
        if !is_installed() {
            return Err(HookError::FrameworkNotInstalled("pre-commit".to_string()));
        }

        if let Some(ref required) = self.config.version {
            let installed = get_version()?;
            if installed.trim() != required.trim() {
                tracing::info!(
                    event = "git_hooks.pre_commit_version_mismatch",
                    installed = %installed,
                    required = %required,
                );
                self.upgrade(required)?;
            }
        }

        let config_path = self.project_dir.join(&self.config.config);
        if !config_path.exists() {
            return Err(HookError::Config(format!(
                "pre-commit config not found at {}; create `.pre-commit-config.yaml` first",
                config_path.display()
            )));
        }

        let mut cmd = Command::new("pre-commit");
        cmd.arg("install");
        if self.config.install_hooks {
            cmd.arg("--install-hooks");
        }
        cmd.current_dir(&self.project_dir);

        let status = cmd.status().map_err(HookError::Io)?;
        if !status.success() {
            return Err(HookError::InstallFailed(format!(
                "pre-commit install exited with {}",
                status.code().unwrap_or(-1)
            )));
        }

        tracing::info!(
            event = "git_hooks.installed",
            framework = "pre-commit",
            install_hooks = self.config.install_hooks,
        );

        Ok(())
    }

    pub fn update(&self) -> Result<(), HookError> {
        if !is_installed() {
            return Err(HookError::FrameworkNotInstalled("pre-commit".to_string()));
        }

        let status = Command::new("pre-commit")
            .args(["autoupdate"])
            .current_dir(&self.project_dir)
            .status()
            .map_err(HookError::Io)?;
        if !status.success() {
            return Err(HookError::UpdateFailed(format!(
                "pre-commit autoupdate exited with {}",
                status.code().unwrap_or(-1)
            )));
        }

        let status = Command::new("pre-commit")
            .args(["install", "--install-hooks"])
            .current_dir(&self.project_dir)
            .status()
            .map_err(HookError::Io)?;
        if !status.success() {
            return Err(HookError::UpdateFailed(format!(
                "pre-commit reinstall exited with {}",
                status.code().unwrap_or(-1)
            )));
        }

        tracing::info!(event = "git_hooks.updated", framework = "pre-commit");
        Ok(())
    }

    pub fn run(&self, all_files: bool, hook_id: Option<&str>) -> Result<(), HookError> {
        if !is_installed() {
            return Err(HookError::FrameworkNotInstalled("pre-commit".to_string()));
        }

        let mut cmd = Command::new("pre-commit");
        cmd.arg("run");
        if all_files {
            cmd.arg("--all-files");
        }
        if let Some(id) = hook_id {
            cmd.arg(id);
        }
        cmd.current_dir(&self.project_dir);

        let status = cmd.status().map_err(HookError::Io)?;
        if !status.success() {
            return Err(HookError::RunFailed(format!(
                "pre-commit run exited with {}",
                status.code().unwrap_or(-1)
            )));
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<HookInfo>, HookError> {
        let config_path = self.project_dir.join(&self.config.config);
        if !config_path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&config_path)?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| HookError::Config(format!("parse {}: {e}", config_path.display())))?;

        let mut hooks = Vec::new();
        if let Some(repos) = yaml.get("repos").and_then(|r| r.as_sequence()) {
            for repo in repos {
                let repo_url = repo
                    .get("repo")
                    .and_then(|r| r.as_str())
                    .unwrap_or("local")
                    .to_string();
                let rev = repo
                    .get("rev")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(repo_hooks) = repo.get("hooks").and_then(|h| h.as_sequence()) {
                    for hook in repo_hooks {
                        if let Some(id) = hook.get("id").and_then(|i| i.as_str()) {
                            hooks.push(HookInfo {
                                id: id.to_string(),
                                repo: repo_url.clone(),
                                version: rev.clone(),
                                hook_type: "pre-commit".to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(hooks)
    }

    fn upgrade(&self, version: &str) -> Result<(), HookError> {
        // pre-commit ships as a Python pkg; pip is the upstream-blessed
        // install path. If pip isn't on PATH, defer to the user — we
        // intentionally don't try `pipx` or `uv` here to keep the
        // failure mode predictable.
        let status = Command::new("pip")
            .args(["install", "--upgrade", &format!("pre-commit=={version}")])
            .status()
            .map_err(HookError::Io)?;
        if !status.success() {
            return Err(HookError::InstallFailed(format!(
                "pip install pre-commit=={version} exited with {}",
                status.code().unwrap_or(-1)
            )));
        }
        Ok(())
    }
}

fn is_installed() -> bool {
    Command::new("pre-commit")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn get_version() -> Result<String, HookError> {
    let output = Command::new("pre-commit")
        .arg("--version")
        .output()
        .map_err(HookError::Io)?;
    let s = String::from_utf8_lossy(&output.stdout);
    // Output shape: `pre-commit 3.6.0`
    Ok(s.split_whitespace().nth(1).unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn list_returns_empty_when_no_config() {
        let tmp = tempdir().unwrap();
        let handler = PreCommitHandler::new(PreCommitConfig::default(), tmp.path().to_path_buf());
        let hooks = handler.list().unwrap();
        assert!(hooks.is_empty());
    }

    #[test]
    fn list_parses_repos_block() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join(".pre-commit-config.yaml"),
            r#"
repos:
  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v4.5.0
    hooks:
      - id: trailing-whitespace
      - id: end-of-file-fixer
  - repo: https://github.com/psf/black
    rev: 24.1.0
    hooks:
      - id: black
"#,
        )
        .unwrap();
        let handler = PreCommitHandler::new(PreCommitConfig::default(), tmp.path().to_path_buf());
        let hooks = handler.list().unwrap();
        assert_eq!(hooks.len(), 3);
        assert_eq!(hooks[0].id, "trailing-whitespace");
        assert_eq!(
            hooks[0].repo,
            "https://github.com/pre-commit/pre-commit-hooks"
        );
        assert_eq!(hooks[0].version, "v4.5.0");
        assert_eq!(hooks[2].id, "black");
    }

    #[test]
    fn list_rejects_malformed_yaml() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join(".pre-commit-config.yaml"),
            "not: valid: yaml: at all:",
        )
        .unwrap();
        let handler = PreCommitHandler::new(PreCommitConfig::default(), tmp.path().to_path_buf());
        let err = handler.list().expect_err("malformed yaml must error");
        match err {
            HookError::Config(_) => {}
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[test]
    fn install_errors_when_config_missing() {
        // Only meaningful if pre-commit is actually installed. Skip
        // otherwise — the outer guard fires first.
        if !is_installed() {
            return;
        }
        let tmp = tempdir().unwrap();
        let handler = PreCommitHandler::new(PreCommitConfig::default(), tmp.path().to_path_buf());
        let err = handler.install().expect_err("missing config must error");
        match err {
            HookError::Config(_) => {}
            other => panic!("expected Config error, got {other:?}"),
        }
    }
}
