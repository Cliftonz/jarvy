//! Remote tool-registry pull
//!
//! Lets a user subscribe to a curated tool-definition registry hosted on
//! HTTPS (typically a GitHub repo). The flow:
//!
//! 1. User writes `[registry] url = "..."` + signing identity into
//!    `~/.jarvy/config.toml`.
//! 2. `jarvy registry sync` fetches `manifest.json` + cosign companions
//!    from the registry root, verifies the manifest signature, then
//!    fetches each referenced tool TOML, sha256-verifies it against the
//!    manifest entry, and caches it under
//!    `~/.jarvy/tools.d/.remote/tools/`.
//! 3. The existing plugin loader (`crate::tools::plugins`) walks the
//!    cache on every `jarvy setup` / `jarvy validate` startup and
//!    registers the synced tools alongside the built-in registry.
//!
//! ## Trust model
//!
//! The registry config lives ONLY in `~/.jarvy/config.toml` (global, user-
//! owned). A remote-fetched project `jarvy.toml` cannot subscribe to a
//! registry — the section is not parsed from project config. This matches
//! the trust-narrowing contract documented in CLAUDE.md and prevents a
//! hostile project config from pointing the runtime at an attacker
//! registry.
//!
//! Each registry config also pins the expected Sigstore signing identity
//! (regexp + OIDC issuer). The default points at the canonical
//! `bearbinary/jarvy-tools` repo's release workflow; self-hosted
//! registries must update both fields. `require_signature = false`
//! exists as an escape hatch for development but is documented as
//! unsafe — Jarvy emits a stderr warning every sync when it's set.

pub mod cache;
pub mod config;
pub mod fetch;
pub mod manifest;
pub mod sync;

#[allow(unused_imports)] // re-exports for crate-internal callers
pub use config::RegistryConfig;
#[allow(unused_imports)]
pub use manifest::{Manifest, ManifestError, ToolEntry};
#[allow(unused_imports)]
pub use sync::{SyncError, SyncReport, run_sync};
