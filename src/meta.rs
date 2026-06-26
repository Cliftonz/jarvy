//! Project-level identity constants.
//!
//! Single source of truth for the repo slug and canonical URLs that
//! would otherwise drift across docs, CI workflows, and source files.
//! When the project moves orgs / renames, only this file changes.
//!
//! Non-Rust consumers (docs, helm charts, GitHub workflows) still hold
//! their own copies — sweeping those is a separate cleanup. The goal of
//! this module is to stop the count of Rust-side hardcodes from growing.

#![allow(dead_code)] // Public API consumed across the crate.

/// `org/repo` slug used in GitHub URLs (issues, PRs, releases).
///
/// **Case matters.** GitHub redirects on the org segment but the path
/// segment is case-sensitive in some downstream contexts (e.g., raw
/// content URLs, Sigstore Fulcio certificate SAN matching). The rest
/// of the repo uses lowercase `Cliftonz/jarvy` — keep this constant
/// aligned with that convention.
pub const REPO_SLUG: &str = "Cliftonz/jarvy";

/// Base repo URL — `https://github.com/<slug>`.
pub const REPO_URL: &str = "https://github.com/Cliftonz/jarvy";
