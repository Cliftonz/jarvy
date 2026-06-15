//! age - Simple, modern, and secure file encryption
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AGE, {
    command: "age",
    macos: { brew: "age" },
    linux: { uniform: "age" },
    windows: { winget: "FiloSottile.age" },
    bsd: { pkg: "age" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_registration_shape() {
        assert_eq!(AGE.command, "age");
        let mac = AGE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("age"));
        let win = AGE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("FiloSottile.age"));
    }
}
