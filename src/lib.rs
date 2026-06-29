// Re-export public modules for use by integration tests and external crates
pub mod agents;
pub mod ai_hooks;
pub mod ci;
pub mod discover;
pub mod drift;
pub mod error_codes;
pub mod git;
pub mod git_hooks;
pub mod library_registry;
pub mod logging;
pub mod mcp_register;
// Internal — `REPO_SLUG` / `REPO_URL` are crate-private. If a future
// workspace consumer (cargo-jarvy, sub-crates) needs them, promote to
// `pub mod meta`. Today only `tools::unsupported` reads them.
pub(crate) mod meta;
pub mod net;
pub mod network;
pub mod observability;
pub mod packages;
pub mod paths;
pub mod progress;
pub mod registry_remote;
pub mod sandbox;
pub mod security;
pub mod skills;
pub mod ticket;
pub mod tools;
pub mod update;
pub mod workspace;

pub use drift::{DriftConfig, DriftDetector, DriftReport, DriftStatus, EnvironmentState};
pub use logging::{LogConfig, LogError, LogFormat, LogLevel, LogStats, Sanitizer};
pub use packages::PackagesConfig;
pub use ticket::{TicketData, TicketError, TicketScope};
pub use tools::{add, register_all};
