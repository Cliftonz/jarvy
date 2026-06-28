//! `jarvy hooks <action>` command handler (PRD-048).
//!
//! Thin glue: loads the `[git_hooks]` block from `jarvy.toml`, dispatches
//! to `crate::git_hooks::{install,update,run,list,hook_status}`, formats
//! the result for stdout, returns an exit code.

use crate::cli::HooksAction;
use crate::config::Config;
use crate::git_hooks::{self, GitHooksConfig};
use crate::progress::Progress;
use std::path::Path;

pub fn run_hooks(action: &HooksAction, file: &str) -> i32 {
    let config = Config::new(file);
    let git_hooks_config = config.git_hooks.clone().unwrap_or_default();
    let project_dir = Path::new(file)
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    match action {
        HooksAction::Install {} => install_action(&git_hooks_config, &project_dir),
        HooksAction::Update {} => update_action(&git_hooks_config, &project_dir),
        HooksAction::Status {} => status_action(&git_hooks_config, &project_dir),
        HooksAction::List {} => list_action(&git_hooks_config, &project_dir),
        HooksAction::Run { all_files, hook } => {
            run_action(&git_hooks_config, &project_dir, *all_files, hook.as_deref())
        }
        HooksAction::Uninstall {} => uninstall_action(&project_dir),
    }
}

fn install_action(config: &GitHooksConfig, project_dir: &Path) -> i32 {
    let progress = Progress::start();
    let spinner = progress.add("[hooks]", "Installing git hooks...");
    match git_hooks::install_hooks(config, project_dir) {
        Ok(true) => {
            spinner.finish_ok("git hooks installed");
            0
        }
        Ok(false) => {
            spinner.finish_skipped("nothing configured");
            println!(
                "  No hook framework detected and none pinned in [git_hooks]. \
                 Add a `.pre-commit-config.yaml` or set `framework = \"pre-commit\"`."
            );
            0
        }
        Err(e) => {
            spinner.finish_failed(format!("{e}"));
            crate::error_codes::HOOK_FAILED
        }
    }
}

fn update_action(config: &GitHooksConfig, project_dir: &Path) -> i32 {
    let progress = Progress::start();
    let spinner = progress.add("[hooks]", "Updating git hooks...");
    match git_hooks::update_hooks(config, project_dir) {
        Ok(true) => {
            spinner.finish_ok("hooks updated");
            0
        }
        Ok(false) => {
            spinner.finish_skipped("nothing configured");
            0
        }
        Err(e) => {
            spinner.finish_failed(format!("{e}"));
            crate::error_codes::HOOK_FAILED
        }
    }
}

fn status_action(config: &GitHooksConfig, project_dir: &Path) -> i32 {
    let status = git_hooks::hook_status(config, project_dir);
    println!("Git Hooks Status");
    println!("================");
    match status.framework {
        Some(f) => println!("Framework:    {}", f.as_str()),
        None => println!("Framework:    (none detected)"),
    }
    println!(
        "Installed:    {}",
        if status.installed { "yes" } else { "no" }
    );
    if let Some(path) = status.config_path {
        println!("Config:       {path}");
    }
    println!("Hook count:   {}", status.hook_count);
    0
}

fn list_action(config: &GitHooksConfig, project_dir: &Path) -> i32 {
    match git_hooks::list_hooks(config, project_dir) {
        Ok(hooks) if hooks.is_empty() => {
            println!("No hooks configured.");
            0
        }
        Ok(hooks) => {
            println!("Configured hooks ({}):", hooks.len());
            // Group by repo for readability.
            let mut current_repo = String::new();
            for h in &hooks {
                if h.repo != current_repo {
                    println!();
                    if h.repo == "local" {
                        println!("  local");
                    } else {
                        println!("  {} ({})", h.repo, h.version);
                    }
                    current_repo.clone_from(&h.repo);
                }
                println!("    {}", h.id);
            }
            0
        }
        Err(e) => {
            eprintln!("Failed to list hooks: {e}");
            crate::error_codes::CONFIG_ERROR
        }
    }
}

fn run_action(
    config: &GitHooksConfig,
    project_dir: &Path,
    all_files: bool,
    hook: Option<&str>,
) -> i32 {
    // `pre-commit run` streams its own output. Skip the progress
    // spinner — it would clash with the subprocess's stdout.
    match git_hooks::run_hooks(config, project_dir, all_files, hook) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Hook run failed: {e}");
            crate::error_codes::HOOK_FAILED
        }
    }
}

fn uninstall_action(project_dir: &Path) -> i32 {
    // `pre-commit uninstall` is the only uninstall path supported today.
    // Bypass the handler abstraction — `update_hooks` etc. require a
    // framework decision but uninstall doesn't need one.
    use std::process::Command;
    let status = Command::new("pre-commit")
        .arg("uninstall")
        .current_dir(project_dir)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("pre-commit hooks uninstalled");
            0
        }
        Ok(s) => {
            eprintln!(
                "pre-commit uninstall exited with {}",
                s.code().unwrap_or(-1)
            );
            crate::error_codes::HOOK_FAILED
        }
        Err(e) => {
            eprintln!("Failed to invoke `pre-commit uninstall`: {e}");
            crate::error_codes::HOOK_FAILED
        }
    }
}
