//! nss - Mozilla NSS tools (certutil for managing NSS certificate databases)
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NSS, {
    command: "certutil",
    macos: { brew: "nss" },
    linux: { apt: "libnss3-tools", dnf: "nss-tools", yum: "nss-tools", zypper: "mozilla-nss-tools", pacman: "nss", apk: "nss-tools" },
    // No Windows block: Windows ships an unrelated certutil.exe.
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nss_registration_shape() {
        assert_eq!(NSS.command, "certutil");
        assert_eq!(NSS.category, Some("networking"));
        let mac = NSS.macos.expect("nss must support macOS");
        assert_eq!(mac.brew, Some("nss"));
        let linux = NSS.linux.expect("nss must support Linux");
        assert_eq!(linux.apt, Some("libnss3-tools"));
        assert_eq!(linux.dnf, Some("nss-tools"));
        assert_eq!(linux.yum, Some("nss-tools"));
        assert_eq!(linux.zypper, Some("mozilla-nss-tools"));
        assert_eq!(linux.pacman, Some("nss"));
        assert_eq!(linux.apk, Some("nss-tools"));
        assert!(NSS.windows.is_none(), "windows certutil.exe is unrelated");
    }
}
