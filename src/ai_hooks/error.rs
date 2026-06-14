//! Error type for AI hook provisioning.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiHookError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse existing settings at {path}: {source}")]
    ParseExisting {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to serialize settings: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("unknown built-in hook '{0}' — run `jarvy ai-hooks list --library`")]
    UnknownLibraryHook(String),

    #[error(
        "agent '{agent}' does not support event '{event}' — see `jarvy ai-hooks list --library` for compatibility"
    )]
    UnsupportedEvent { agent: &'static str, event: String },

    #[error("agent '{0}' is not supported on this platform (Cline hooks are macOS/Linux only)")]
    UnsupportedPlatform(&'static str),

    #[error("invalid hook entry '{name}': {reason}")]
    InvalidEntry { name: String, reason: String },

    #[error(
        "settings file at {path} is a symlink — refusing to write through it. Remove the symlink or set `JARVY_HOME` to a clean location."
    )]
    SettingsPathIsSymlink { path: PathBuf },
}

impl AiHookError {
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Stable telemetry tag identifying which variant fired. Used by
    /// `ai_hook.failed` events so dashboards can group by error type
    /// without serializing the human-readable message.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io { .. } => "io",
            Self::ParseExisting { .. } => "parse_existing",
            Self::Serialize(_) => "serialize",
            Self::UnknownLibraryHook(_) => "unknown_library_hook",
            Self::UnsupportedEvent { .. } => "unsupported_event",
            Self::UnsupportedPlatform(_) => "unsupported_platform",
            Self::InvalidEntry { .. } => "invalid_entry",
            Self::SettingsPathIsSymlink { .. } => "settings_path_is_symlink",
        }
    }

    /// Some variants are routine configuration errors (typo'd library
    /// name, unsupported event/platform). These should not page on-call.
    /// Reserved for use by the telemetry layer once routine-vs-incident
    /// routing lands; currently informational.
    #[allow(dead_code)]
    pub fn is_routine(&self) -> bool {
        matches!(
            self,
            Self::UnknownLibraryHook(_)
                | Self::UnsupportedEvent { .. }
                | Self::UnsupportedPlatform(_)
                | Self::InvalidEntry { .. }
        )
    }
}
