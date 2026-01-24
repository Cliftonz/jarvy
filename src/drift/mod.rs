//! Configuration drift detection module
//!
//! This module provides functionality to detect when a developer's environment
//! has drifted from the expected configuration defined in `jarvy.toml`:
//!
//! - State capture after setup
//! - Drift detection (version changes, missing tools, file changes)
//! - Report generation (text and JSON)
//! - Remediation support
//!
//! Configuration is read from the `[drift]` section of `jarvy.toml`.

mod config;
mod detector;
mod fixer;
mod reporter;
pub mod state;

#[allow(unused_imports)]
pub use config::{DriftConfig, VersionPolicy};
#[allow(unused_imports)]
pub use detector::{
    ChangedFile, DriftDetector, DriftReport, DriftStatus, DriftSummary, ExtraTool, MissingTool,
    VersionChange, VersionDirection,
};
#[allow(unused_imports)]
pub use fixer::{DriftFixer, FixResult, FixStatus};
pub use reporter::DriftReporter;
#[allow(unused_imports)]
pub use state::{EnvironmentState, ToolState};

use thiserror::Error;

/// Errors that can occur during drift detection
#[derive(Debug, Error)]
pub enum DriftError {
    #[error("Failed to read state file: {0}")]
    StateReadError(#[from] std::io::Error),

    #[error("Failed to parse state file: {0}")]
    StateParseError(#[from] serde_json::Error),

    #[allow(dead_code)]
    #[error("No baseline state found. Run 'jarvy setup' first.")]
    NoBaseline,

    #[allow(dead_code)]
    #[error("Failed to detect tool version: {0}")]
    VersionDetectionError(String),

    #[error("Failed to hash file: {0}")]
    HashError(String),
}
