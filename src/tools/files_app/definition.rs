//! files_app - Files (files-community/Files), modern open-source file manager
//! for Windows 11/10 with tabs, dual panes, and color tags.
//!
//! Windows: winget (`FilesCommunity.Files`). Microsoft Store variant
//! (`9NGHP3DX8HDX`) also exists but the winget package is the community
//! distribution and installs the unpackaged app. macOS/Linux: never released.

use crate::define_tool;

define_tool!(FILES_APP, {
    command: "files",
    windows: { winget: "FilesCommunity.Files" },
    category: "file_manager",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_app_registration_shape() {
        assert_eq!(FILES_APP.command, "files");
        assert_eq!(
            FILES_APP.windows.unwrap().winget,
            Some("FilesCommunity.Files")
        );
        assert!(FILES_APP.macos.is_none());
        assert!(FILES_APP.linux.is_none());
        assert_eq!(FILES_APP.category, Some("file_manager"));
    }
}
