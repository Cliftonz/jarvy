//! dust - a more intuitive version of du
//!
//! dust is a modern replacement for du written in Rust.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DUST, {
    command: "dust",
    repo: "bootandy/dust",
    macos: { brew: "dust" },
    linux: { apt: "du-dust", dnf: "dust", pacman: "dust", apk: "dust" },
    windows: { winget: "bootandy.dust", choco: "dust" },
    bsd: { pkg: "dust" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dust_registration_shape() {
        assert_eq!(DUST.command, "dust");
        let mac = DUST.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("dust"));
        let win = DUST.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("bootandy.dust"));
    }
}
