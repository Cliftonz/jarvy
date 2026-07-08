//! PuTTY - SSH and Telnet client
//!
//! `putty` is the classic SSH/Telnet client suite (putty, plink,
//! pscp, psftp, puttygen). Best known on Windows, but packaged under
//! the same name in homebrew-core and every major Linux distro.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PUTTY, {
    command: "putty",
    macos: { brew: "putty" },
    linux: { uniform: "putty" },
    windows: { winget: "PuTTY.PuTTY" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn putty_registration_shape() {
        assert_eq!(PUTTY.command, "putty");
        assert_eq!(PUTTY.category, Some("networking"));
        let mac = PUTTY.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("putty"));
        let linux = PUTTY.linux.expect("must support Linux");
        assert_eq!(linux.apt, Some("putty"));
        let win = PUTTY.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("PuTTY.PuTTY"));
    }
}
