//! CLI command implementations for Jarvy
//!
//! This module contains implementations for all CLI commands:
//! - `jarvy setup` - Environment setup (setup_cmd)
//! - `jarvy bootstrap` - Minimal machine bootstrap (bootstrap_cmd)
//! - `jarvy configure` - Generate default config (configure_cmd)
//! - `jarvy get` - Display tool status (get)
//! - `jarvy tools` - List supported tools (tools_cmd)
//! - `jarvy env` - Manage environment variables (env_cmd)
//! - `jarvy ci-config` / `jarvy ci-info` - CI configuration (ci_cmd)
//! - `jarvy services` - Manage services (services_cmd)
//! - `jarvy mcp` - MCP server (mcp_cmd)
//! - `jarvy telemetry` - Telemetry settings (telemetry_cmd)
//! - `jarvy team` - Team configuration sources (team_cmd)
//! - `jarvy roles` - Role-based configurations (roles_cmd)
//! - `jarvy lock` - Version lock files (lock_cmd)
//! - `jarvy config` - Configuration inheritance (config_cmd)
//! - `jarvy doctor` - Environment diagnostics
//! - `jarvy diagnose` - Deep tool diagnosis (PRD-027)
//! - `jarvy diff` - Preview changes before setup
//! - `jarvy export` - Generate jarvy.toml from installed tools
//! - `jarvy init` - Interactive project initialization (PRD-023)
//! - `jarvy upgrade` - Upgrade tools to latest versions
//! - `jarvy search` - Search available tools
//! - `jarvy validate` - Validate configuration files
//! - `jarvy completions` - Generate shell completions

// Existing command modules
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

// New command modules extracted from main.rs (PRD-037)
pub mod bootstrap_cmd;
pub mod ci_cmd;
pub mod config_cmd;
pub mod configure_cmd;
pub mod drift_cmd;
pub mod env_cmd;
pub mod get;
pub mod lock_cmd;
pub mod mcp_cmd;
pub mod roles_cmd;
pub mod services_cmd;
pub mod setup_cmd;
pub mod team_cmd;
pub mod telemetry_cmd;
pub mod tools_cmd;

// Public API re-exports - these modules may not be used directly by main.rs
// but are part of the commands module's public interface
#[allow(unused_imports)]
pub use completions::*;
#[allow(unused_imports)]
pub use diagnose::*;
#[allow(unused_imports)]
pub use diff::*;
#[allow(unused_imports)]
pub use doctor::*;
#[allow(unused_imports)]
pub use export::*;
#[allow(unused_imports)]
pub use init::*;
#[allow(unused_imports)]
pub use quickstart::*;
#[allow(unused_imports)]
pub use search::*;
#[allow(unused_imports)]
pub use templates::*;
#[allow(unused_imports)]
pub use upgrade::*;
#[allow(unused_imports)]
pub use validate::*;

// Re-exports for new command modules
pub use bootstrap_cmd::*;
pub use ci_cmd::*;
pub use config_cmd::*;
pub use configure_cmd::*;
pub use drift_cmd::*;
pub use env_cmd::*;
pub use get::*;
pub use lock_cmd::*;
pub use mcp_cmd::*;
pub use roles_cmd::*;
pub use services_cmd::*;
pub use team_cmd::*;
pub use telemetry_cmd::*;
pub use tools_cmd::*;
