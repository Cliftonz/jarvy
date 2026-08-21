//! directory_opus - Directory Opus (GPSoftware), powerful highly-configurable
//! Explorer replacement for advanced Windows users. Commercial (paid license).
//!
//! Windows: winget (`GPSoftware.DirectoryOpus`). macOS/Linux: not released.

use crate::define_tool;

define_tool!(DIRECTORY_OPUS, {
    command: "dopus",
    windows: { winget: "GPSoftware.DirectoryOpus" },
    category: "file_manager",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_opus_registration_shape() {
        assert_eq!(DIRECTORY_OPUS.command, "dopus");
        assert_eq!(
            DIRECTORY_OPUS.windows.unwrap().winget,
            Some("GPSoftware.DirectoryOpus")
        );
        assert!(DIRECTORY_OPUS.macos.is_none());
        assert!(DIRECTORY_OPUS.linux.is_none());
        assert_eq!(DIRECTORY_OPUS.category, Some("file_manager"));
    }
}
