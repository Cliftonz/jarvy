//! total_commander - Total Commander (Ghisler), classic dual-pane file
//! manager built for high-speed bulk management. Shareware.
//!
//! Windows: winget (`Ghisler.TotalCommander`). macOS/Linux: not released
//! (though Wine builds exist unofficially).

use crate::define_tool;

define_tool!(TOTAL_COMMANDER, {
    command: "totalcmd",
    windows: { winget: "Ghisler.TotalCommander" },
    category: "file_manager",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_commander_registration_shape() {
        assert_eq!(TOTAL_COMMANDER.command, "totalcmd");
        assert_eq!(
            TOTAL_COMMANDER.windows.unwrap().winget,
            Some("Ghisler.TotalCommander")
        );
        assert!(TOTAL_COMMANDER.macos.is_none());
        assert!(TOTAL_COMMANDER.linux.is_none());
        assert_eq!(TOTAL_COMMANDER.category, Some("file_manager"));
    }
}
