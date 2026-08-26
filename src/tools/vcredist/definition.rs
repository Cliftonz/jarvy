//! vcredist - Microsoft Visual C++ 2015-2022 Redistributable (x64). Runtime
//! DLLs (vcruntime140.dll, msvcp140.dll, ...) that many Windows tools link
//! against — including `git` when installed via winget on a fresh box.
//!
//! Add BEFORE `git` in jarvy.toml if a fresh Windows box hits "missing
//! library" errors from winget-installed tools.
//!
//! Windows: winget (`Microsoft.VCRedist.2015+.x64`) / choco (`vcredist140`).
//! macOS/Linux: not applicable.

use crate::define_tool;

define_tool!(VCREDIST, {
    command: "vcredist",
    windows: { winget: "Microsoft.VCRedist.2015+.x64", choco: "vcredist140" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vcredist_registration_shape() {
        assert_eq!(VCREDIST.command, "vcredist");
        let win = VCREDIST.windows.unwrap();
        assert_eq!(win.winget, Some("Microsoft.VCRedist.2015+.x64"));
        assert_eq!(win.choco, Some("vcredist140"));
        assert!(VCREDIST.macos.is_none());
        assert!(VCREDIST.linux.is_none());
    }
}
