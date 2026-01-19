//! zsh - Z shell
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively - use WSL or MSYS2.

use crate::define_tool;

define_tool!(ZSH, {
    command: "zsh",
    macos: { brew: "zsh" },
    linux: { uniform: "zsh" },
    // No Windows support - zsh is Unix-only
    bsd: { pkg: "zsh" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_zsh_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
