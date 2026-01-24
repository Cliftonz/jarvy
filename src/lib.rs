// Re-export public modules for use by integration tests and external crates
pub mod drift;
pub mod git;
pub mod network;
pub mod packages;
pub mod tools;

pub use drift::{DriftConfig, DriftDetector, DriftReport, DriftStatus, EnvironmentState};
pub use packages::PackagesConfig;
pub use tools::{add, register_all};
