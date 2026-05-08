//! Shared HTTP / network primitives. Currently a single-module wrapper
//! around `ureq::Agent` with timeouts, but the right home for any future
//! cross-cutting network helpers (URL allowlist, redirect callback, etc.).

pub mod agent;

pub use agent::{agent, user_agent};
