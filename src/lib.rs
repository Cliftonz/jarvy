// Re-export public modules for use by integration tests and external crates
pub mod drift;
pub mod git;
pub mod logging;
pub mod net;
pub mod network;
pub mod observability;
pub mod packages;
pub mod security;
pub mod ticket;
pub mod tools;
pub mod workspace;

pub use drift::{DriftConfig, DriftDetector, DriftReport, DriftStatus, EnvironmentState};
pub use logging::{LogConfig, LogError, LogFormat, LogLevel, LogStats, Sanitizer};
pub use packages::PackagesConfig;
pub use ticket::{TicketData, TicketError, TicketScope};
pub use tools::{add, register_all};
