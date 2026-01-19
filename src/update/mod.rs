//! Self-updating functionality for Jarvy CLI
//!
//! This module provides automatic update checking and installation via:
//! - Multiple installation methods (Homebrew, Cargo, apt, dnf, winget, etc.)
//! - Background update checking with throttling
//! - Secure binary downloads with checksum verification
//! - Rollback support for failed updates

pub mod checker;
pub mod commands;
pub mod config;
pub mod installer;
pub mod method;
pub mod release;
pub mod rollback;
pub mod signature;

pub use checker::{CURRENT_VERSION, CheckResult, UpdateChecker, UpdateState};
pub use commands::{UpdateAction, run_update_command, show_update_notification_if_available};
pub use config::{Channel, UpdateConfig};
pub use installer::BinaryInstaller;
pub use method::{InstallMethod, UpdateError};
pub use release::{GitHubRelease, ReleaseAsset, ReleaseClient};
pub use rollback::{RollbackInfo, RollbackManager, RollbackResult};
