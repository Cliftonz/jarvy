//! cue - CUE configuration language CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively.

use crate::define_tool;

define_tool!(CUE, {
    command: "cue",
    macos: { brew: "cue" },
    linux: { uniform: "cue" },
    bsd: { pkg: "cue" },
    // No first-party winget manifest; the go fallback route covers
    // Windows (verified 2026-08).
    fallback: { go: "cuelang.org/go/cmd/cue" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_registration_shape() {
        assert_eq!(CUE.command, "cue");
        let mac = CUE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("cue"));
        assert!(CUE.windows.is_none(), "no first-party winget manifest");
        assert_eq!(CUE.fallback.len(), 1);
        assert_eq!(CUE.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(CUE.fallback[0].package, "cuelang.org/go/cmd/cue");
    }
}
