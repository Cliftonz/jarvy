//! CLI command implementations for PRD-016 Developer Experience Commands
//!
//! This module contains implementations for:
//! - `jarvy doctor` - Environment diagnostics
//! - `jarvy diagnose` - Deep tool diagnosis (PRD-027)
//! - `jarvy diff` - Preview changes before setup
//! - `jarvy export` - Generate jarvy.toml from installed tools
//! - `jarvy init` - Interactive project initialization (PRD-023)
//! - `jarvy upgrade` - Upgrade tools to latest versions
//! - `jarvy search` - Search available tools
//! - `jarvy validate` - Validate configuration files
//! - `jarvy completions` - Generate shell completions

pub mod completions;
pub mod diagnose;
pub mod diff;
pub mod doctor;
pub mod export;
pub mod init;
pub mod quickstart;
pub mod search;
pub mod templates;
pub mod upgrade;
pub mod validate;

pub use completions::*;
pub use diagnose::*;
pub use diff::*;
pub use doctor::*;
pub use export::*;
pub use init::*;
pub use quickstart::*;
pub use search::*;
pub use templates::*;
pub use upgrade::*;
pub use validate::*;
