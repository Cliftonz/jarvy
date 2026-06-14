//! grpcurl - command-line tool for interacting with gRPC servers
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GRPCURL, {
    command: "grpcurl",
    macos: { brew: "grpcurl" },
    linux: { uniform: "grpcurl" },
    // Canonical winget PackageIdentifier is `fullstorydev.grpcurl` —
    // see `winget-pkgs/manifests/f/fullstorydev/grpcurl/`. The publisher
    // segment is case-insensitive on current winget releases, but we
    // match exactly to avoid typo-squatting exposure if a different
    // publisher ever registers an exact-case `FullStory.grpcurl`.
    windows: { winget: "fullstorydev.grpcurl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the registration shape — not the tautology that
    /// `ensure` returns a `Result`. A typo in `command` would silently
    /// mean `jarvy` can't detect whether grpcurl is installed; a typo
    /// in the winget id is the supply-chain regression Codex caught
    /// once already.
    #[test]
    fn grpcurl_registration_shape() {
        assert_eq!(GRPCURL.command, "grpcurl");
        // macOS: brew formula "grpcurl" (the canonical Homebrew source).
        let macos = GRPCURL.macos.expect("grpcurl must support macOS");
        assert_eq!(
            macos.brew,
            Some("grpcurl"),
            "macOS spec must use brew 'grpcurl'"
        );
        // Windows: canonical lowercase winget publisher segment.
        let windows = GRPCURL.windows.expect("grpcurl must support Windows");
        assert_eq!(
            windows.winget,
            Some("fullstorydev.grpcurl"),
            "winget id must be the canonical lowercase fullstorydev.grpcurl, not FullStory.grpcurl"
        );
    }
}
