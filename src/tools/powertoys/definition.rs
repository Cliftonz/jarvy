//! powertoys - Microsoft PowerToys, utility suite for Windows power users.
//! Includes FancyZones (window tiling), PowerRename, PowerToys Run, Keyboard
//! Manager, Awake, Color Picker, and 25+ other utilities in one install.
//!
//! Windows: winget (`Microsoft.PowerToys`) / choco (`powertoys`). macOS/Linux:
//! not released.

use crate::define_tool;

define_tool!(POWERTOYS, {
    command: "PowerToys",
    windows: { winget: "Microsoft.PowerToys", choco: "powertoys" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powertoys_registration_shape() {
        assert_eq!(POWERTOYS.command, "PowerToys");
        let win = POWERTOYS.windows.unwrap();
        assert_eq!(win.winget, Some("Microsoft.PowerToys"));
        assert_eq!(win.choco, Some("powertoys"));
        assert!(POWERTOYS.macos.is_none());
        assert!(POWERTOYS.linux.is_none());
    }
}
