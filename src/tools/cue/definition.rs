//! cue - CUE configuration language CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively.

use crate::define_tool;

define_tool!(CUE, {
    command: "cue",
    repo: "cue-lang/cue",
    macos: { brew: "cue" },
    linux: { uniform: "cue" },
    bsd: { pkg: "cue" },
    // No Windows support
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_registration_shape() {
        assert_eq!(CUE.command, "cue");
        let mac = CUE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("cue"));
    }
}
