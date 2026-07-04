//! vfox - cross-platform version manager
//!
//! A cross-platform and extendable version manager with support for
//! Java, Node.js, Golang, Python, Flutter, .NET & more.
//!
//! See: https://github.com/version-fox/vfox

use crate::define_tool;

define_tool!(VFOX, {
    command: "vfox",
    repo: "version-fox/vfox",
    macos: { brew: "vfox" },
    linux: { brew: "vfox" },
    windows: { winget: "vfox" },
    bsd: { pkg: "vfox" },
    default_hook_shell_init: ("vfox", "activate"),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfox_registration_shape() {
        assert_eq!(VFOX.command, "vfox");
        let mac = VFOX.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("vfox"));
        let win = VFOX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("vfox"));
    }
}
