//! surreal — SurrealDB command-line tool and server.
//!
//! Homepage: <https://surrealdb.com>. A single executable containing
//! both the `surreal` CLI (start / sql / import / export / upgrade) and
//! the embedded SurrealDB server, for the realtime document-graph
//! database.
//!
//! Install paths:
//! - macOS: Homebrew formula `surrealdb/tap/surreal`. The two-slash
//!   `org/tap/formula` id triggers the macro's auto-`brew tap` so a
//!   fresh box doesn't surface an "untrusted tap" error.
//! - Linux: same tap via Homebrew-on-Linux (mirrors `dbmate`). No
//!   first-party apt/dnf packaging exists — upstream ships an install
//!   script (`curl -sSf https://install.surrealdb.com | sh`) instead.
//! - Windows: Chocolatey package `surreal` (published + auto-updated
//!   by SurrealDB via GitHub Actions CD — <https://github.com/surrealdb/chocolatey>).
//!   Note the id is `surreal`, NOT `surrealdb`.
//!
//! No first-party winget manifest as of 2026-07; install from choco or
//! <https://surrealdb.com/install>. Omitting the winget slot avoids
//! pinning `winget install -e --id` to an unclaimed publisher namespace
//! (supply-chain exposure — see the CLAUDE.md "Omit unsupported
//! platforms" rule).

use crate::define_tool;

define_tool!(SURREAL, {
    command: "surreal",
    repo: "surrealdb/surrealdb",
    macos: { brew: "surrealdb/tap/surreal" },
    linux: { brew: "surrealdb/tap/surreal" },
    windows: { choco: "surreal" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surreal_registration_shape() {
        assert_eq!(SURREAL.command, "surreal");
        let mac = SURREAL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("surrealdb/tap/surreal"));
        let lin = SURREAL.linux.expect("must support Linux via brew fallback");
        assert_eq!(lin.brew, Some("surrealdb/tap/surreal"));
        let win = SURREAL.windows.expect("must support Windows via choco");
        assert_eq!(win.choco, Some("surreal"));
    }

    /// No first-party winget manifest — the slot MUST stay empty so we
    /// never pin `winget install -e --id` to an unclaimed publisher.
    #[test]
    fn surreal_omits_winget() {
        let win = SURREAL.windows.expect("windows block present for choco");
        assert_eq!(
            win.winget, None,
            "no verified first-party winget manifest; keep winget unset"
        );
    }
}
