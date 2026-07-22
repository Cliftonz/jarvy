//! CLI commands for self-update functionality
//!
//! Provides commands for checking, installing, and managing updates.

#![allow(dead_code)] // Public API for update commands

use crate::update::checker::{CURRENT_VERSION, CheckResult, UpdateChecker};
use crate::update::config::{Channel, UpdateConfig, is_interactive};
use crate::update::installer::BinaryInstaller;
use crate::update::method::{InstallMethod, UpdateError};
use crate::update::release::ReleaseClient;
use crate::update::rollback::RollbackManager;

/// Update actions dispatched from main CLI
#[derive(Debug, Clone)]
pub enum UpdateAction {
    /// Check for available updates
    Check {
        /// Override release channel for this check
        channel: Option<Channel>,
    },
    /// Install an update
    Install {
        /// Specific version to install
        version: Option<String>,
        /// Override release channel
        channel: Option<Channel>,
        /// Override installation method
        method: Option<InstallMethod>,
        /// Perform rollback instead of install
        rollback: bool,
        /// Operator override: accept unsigned/missing-cosign installs.
        /// Defaults to false (fail-closed).
        allow_unsigned: bool,
    },
    /// Show update history
    History,
    /// Show update configuration
    Config,
    /// Enable automatic updates
    Enable,
    /// Disable automatic updates
    Disable,
}

/// Run the update command and return exit code
pub fn run_update_command(action: UpdateAction) -> i32 {
    let result = match action {
        UpdateAction::Check { channel } => run_check(channel),
        UpdateAction::Install {
            version,
            channel,
            method,
            rollback,
            allow_unsigned,
        } => {
            if rollback {
                run_rollback()
            } else {
                run_install(version, channel, method, allow_unsigned)
            }
        }
        UpdateAction::History => run_history(),
        UpdateAction::Config => run_config(),
        UpdateAction::Enable => run_enable(),
        UpdateAction::Disable => run_disable(),
    };

    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Update error: {}", e);
            1
        }
    }
}

/// Check for updates and show result
fn run_check(channel_override: Option<Channel>) -> Result<(), UpdateError> {
    let mut checker = UpdateChecker::new();

    // Apply channel override if provided
    if let Some(ch) = channel_override {
        // Temporarily use the override channel for this check
        let mut config = checker.config().clone();
        config.channel = ch;
        checker = UpdateChecker::with_config(config);
    }

    println!("Current version: {}", CURRENT_VERSION);
    println!("Channel: {}", checker.config().channel);

    // Spinner across the network check. `Progress` auto-disables in
    // CI / sandboxes / non-TTY (PRD-052) — the println!s below remain
    // the source of truth for log scrapers.
    let progress = crate::progress::Progress::start();
    let spinner = progress.add("[update]", "Checking for updates...");

    let result = checker.check();
    match &result {
        Ok(CheckResult::UpToDate) => spinner.finish_ok("up to date"),
        Ok(CheckResult::UpdateAvailable { latest, .. }) => {
            spinner.finish_ok(format!("update available: {latest}"))
        }
        Err(_) => spinner.finish_failed("check failed"),
    }

    match result {
        Ok(CheckResult::UpToDate) => {
            println!("\nJarvy is up to date!");
            Ok(())
        }
        Ok(CheckResult::UpdateAvailable {
            current,
            latest,
            changelog,
            release_url,
        }) => {
            println!("\nUpdate available!");
            println!("  Current: {}", current);
            println!("  Latest:  {}", latest);

            if let Some(url) = release_url {
                println!("\n  Release notes: {}", url);
            }

            if let Some(log) = changelog {
                println!("\nWhat's new:");
                // Print first few lines of changelog
                for line in log.lines().take(10) {
                    println!("  {}", line);
                }
            }

            println!("\nRun 'jarvy update' to install the update.");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to check for updates: {}", e);
            Err(e.into())
        }
    }
}

/// Install an update
fn run_install(
    version: Option<String>,
    channel: Option<Channel>,
    method: Option<InstallMethod>,
    allow_unsigned: bool,
) -> Result<(), UpdateError> {
    // Determine channel (reserved for future use with channel-specific releases)
    let _channel = channel.unwrap_or(UpdateConfig::load().channel);

    // Determine installation method
    let method = method.unwrap_or_else(InstallMethod::detect);

    println!("Installation method: {}", method);

    // Get target version
    let target_version = if let Some(v) = version {
        v
    } else {
        // First check if update is needed
        let mut checker = UpdateChecker::new();
        match checker.check() {
            Ok(CheckResult::UpToDate) => {
                println!("Jarvy v{} is already up to date!", CURRENT_VERSION);
                return Ok(());
            }
            Ok(CheckResult::UpdateAvailable { latest, .. }) => {
                if !is_interactive() {
                    println!("Update available: {} -> {}", CURRENT_VERSION, latest);
                } else {
                    println!("Update available: {} -> {}", CURRENT_VERSION, latest);
                    println!("Proceed with installation? [Y/n]");

                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).ok();

                    if !input.trim().is_empty() && input.trim().to_lowercase() != "y" {
                        println!("Update cancelled.");
                        return Ok(());
                    }
                }
                latest
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    };

    println!("Updating to version {}...", target_version);

    // Execute update based on method
    if method.supports_direct_update() {
        method.execute_update(Some(&target_version))?;
    } else {
        // Fall back to binary installation
        let client = ReleaseClient::new();
        let release = client
            .fetch_by_tag(&format!("v{}", target_version))
            .map_err(|e| UpdateError::DownloadFailed(e.to_string()))?;

        let installer =
            BinaryInstaller::new().map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;

        installer.install_with_options(&release, allow_unsigned)?;
    }

    println!("\nSuccessfully updated to jarvy v{}", target_version);
    println!("  Run 'jarvy --version' to verify.");

    Ok(())
}

/// Rollback to previous version
fn run_rollback() -> Result<(), UpdateError> {
    let Some(info) = RollbackManager::info() else {
        println!("No rollback available.");
        println!("Rollback is only available immediately after an update.");
        return Ok(());
    };

    if !RollbackManager::can_rollback() {
        println!("No rollback available.");
        println!("Rollback is only available immediately after an update.");
        return Ok(());
    }

    println!(
        "Rolling back from {} to {}...",
        info.new_version, info.previous_version
    );

    let result = RollbackManager::rollback()?;

    println!("\nRolled back to jarvy v{}", result.restored_version);
    println!("  The backup has been consumed and cannot be used again.");

    Ok(())
}

/// Show update history
fn run_history() -> Result<(), UpdateError> {
    // Check rollback info for last update
    if let Some(info) = RollbackManager::info() {
        println!("Last update:");
        println!("  From:    v{}", info.previous_version);
        println!("  To:      v{}", info.new_version);
        println!("  Backup:  {}", info.backup_path.display());
        println!("  Rollback available: yes");
    } else {
        println!("No update history available.");
        println!("History is recorded when updates are installed via 'jarvy update'.");
    }

    Ok(())
}

/// Show current configuration
fn run_config() -> Result<(), UpdateError> {
    let config = UpdateConfig::load();
    let method = InstallMethod::detect();
    let checker = UpdateChecker::new();

    println!("Update Configuration:");
    println!("  Enabled:          {}", config.enabled);
    println!("  Channel:          {}", config.channel);
    println!("  Auto-install:     {:?}", config.auto_install);
    println!("  Check interval:   {:?}", config.check_interval);
    println!("  Patch only:       {}", config.patch_only);
    println!("  Notifications:    {}", config.show_notifications);

    if let Some(ref pin) = config.pinned_version {
        println!("  Pinned version:   {}", pin);
    }

    println!("\nDetected install method: {}", method);
    println!("Current version: {}", CURRENT_VERSION);

    if let Some(last) = checker.state().last_checked {
        let datetime = format_timestamp(last);
        println!("Last checked: {}", datetime);
    }

    if let Some(ref avail) = checker.state().available_version {
        println!("Available version: {}", avail);
    }

    if RollbackManager::can_rollback() {
        println!("\nRollback available: yes");
    }

    Ok(())
}

/// Enable automatic updates
fn run_enable() -> Result<(), UpdateError> {
    let mut config = UpdateConfig::load();
    config.enabled = true;
    config.save().map_err(UpdateError::Io)?;

    println!("Automatic updates enabled.");
    println!("  Jarvy will check for updates periodically.");

    Ok(())
}

/// Disable automatic updates
fn run_disable() -> Result<(), UpdateError> {
    let mut config = UpdateConfig::load();
    config.enabled = false;
    config.save().map_err(UpdateError::Io)?;

    println!("Automatic updates disabled.");
    println!("  You can still manually check with 'jarvy update check'.");

    Ok(())
}

/// Format unix timestamp as human-readable string
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let time = UNIX_EPOCH + Duration::from_secs(timestamp);
    match time.elapsed() {
        Ok(elapsed) => {
            let secs = elapsed.as_secs();
            if secs < 60 {
                "just now".to_string()
            } else if secs < 3600 {
                format!("{} minutes ago", secs / 60)
            } else if secs < 86400 {
                format!("{} hours ago", secs / 3600)
            } else {
                format!("{} days ago", secs / 86400)
            }
        }
        Err(_) => "unknown".to_string(),
    }
}

impl From<crate::update::checker::CheckError> for UpdateError {
    fn from(e: crate::update::checker::CheckError) -> Self {
        UpdateError::DownloadFailed(e.to_string())
    }
}

/// Show update notification if an update is available
/// Called after other commands complete to show non-blocking notification
pub fn show_update_notification_if_available() {
    let mut checker = UpdateChecker::new();

    if checker.should_notify()
        && let Some(msg) = checker.notification_message()
    {
        eprintln!("\n{}", msg);
        checker.mark_notified();
    }
}
