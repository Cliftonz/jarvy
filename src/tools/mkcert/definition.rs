//! mkcert - locally-trusted development certificates
//!
//! Creates a local CA, installs it in the system and browser trust stores,
//! and issues certificates for localhost and other dev hostnames.

use crate::define_tool;
use crate::tools::common::Os;

define_tool!(MKCERT, {
    command: "mkcert",
    macos: { brew: "mkcert" },
    linux: { apt: "mkcert", dnf: "mkcert", pacman: "mkcert" },
    windows: { winget: "FiloSottile.mkcert" },
    fallback: { go: "filippo.io/mkcert" },
    depends_on_by_os: &[(Os::Macos, "nss"), (Os::Linux, "nss")],
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mkcert_registration_shape() {
        assert_eq!(MKCERT.command, "mkcert");
        assert_eq!(MKCERT.category, Some("networking"));
        let mac = MKCERT.macos.expect("mkcert must support macOS");
        assert_eq!(mac.brew, Some("mkcert"));
        let linux = MKCERT.linux.expect("mkcert must support Linux");
        assert_eq!(linux.apt, Some("mkcert"));
        assert_eq!(linux.dnf, Some("mkcert"));
        assert_eq!(linux.pacman, Some("mkcert"));
        assert!(linux.apk.is_none());
        let win = MKCERT.windows.expect("mkcert must support Windows");
        assert_eq!(win.winget, Some("FiloSottile.mkcert"));
        assert_eq!(MKCERT.fallback.len(), 1);
        assert_eq!(MKCERT.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(MKCERT.fallback[0].package, "filippo.io/mkcert");
        assert_eq!(
            MKCERT.depends_on_by_os,
            &[(Os::Macos, "nss"), (Os::Linux, "nss")]
        );
    }
}
