//! choose - human-friendly cut/awk alternative
//!
//! Choose is a human-friendly and fast alternative to cut and (sometimes) awk.
//! It allows selecting fields from lines with simple syntax.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CHOOSE, {
    command: "choose",
    macos: { brew: "choose-rust" },
    linux: { uniform: "choose" },
    windows: { winget: "choose.choose" },
    bsd: { pkg: "choose" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_registration_shape() {
        assert_eq!(CHOOSE.command, "choose");
        let mac = CHOOSE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("choose-rust"));
        let win = CHOOSE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("choose.choose"));
    }
}
