//! onecommander - OneCommander, modern file manager for Windows 11/10 with
//! column layouts and fast previews.
//!
//! Windows: winget (`MilosParipovic.OneCommander`). macOS/Linux: not released.

use crate::define_tool;

define_tool!(ONECOMMANDER, {
    command: "onecommander",
    windows: { winget: "MilosParipovic.OneCommander" },
    category: "file_manager",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onecommander_registration_shape() {
        assert_eq!(ONECOMMANDER.command, "onecommander");
        assert_eq!(
            ONECOMMANDER.windows.unwrap().winget,
            Some("MilosParipovic.OneCommander")
        );
        assert!(ONECOMMANDER.macos.is_none());
        assert!(ONECOMMANDER.linux.is_none());
        assert_eq!(ONECOMMANDER.category, Some("file_manager"));
    }
}
