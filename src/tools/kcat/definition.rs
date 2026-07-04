//! kcat - Apache Kafka swiss-army CLI (was `kafkacat`)
//!
//! `kcat` is the netcat-equivalent for Kafka — produce, consume,
//! list topics, query metadata, debug consumer groups. Tiny single
//! binary, the daily-driver Kafka CLI for most operators. Renamed
//! from `kafkacat` upstream in 2021; brew ships as `kcat` but Debian
//! / Ubuntu stable still ship the binary under `kafkacat`.

use crate::define_tool;

define_tool!(KCAT, {
    command: "kcat",
    repo: "edenhill/kcat",
    macos: { brew: "kcat" },
    // Per-package-manager split: Debian-family stable still packages
    // under the legacy `kafkacat` name; Fedora / Arch / Alpine adopted
    // the new `kcat` name.
    linux: {
        apt: "kafkacat",
        dnf: "kcat",
        pacman: "kcat",
        apk: "kcat"
    },
    // No first-party winget manifest as of 2026-06; the prior
    // `edenhill.kcat` id was never claimed. Windows users: install
    // from https://github.com/edenhill/kcat/releases.
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kcat_registration_shape() {
        assert_eq!(KCAT.command, "kcat");
        assert_eq!(KCAT.category, Some("messaging"));
        let mac = KCAT.macos.expect("kcat must support macOS");
        assert_eq!(
            mac.brew,
            Some("kcat"),
            "post-rename formula is `kcat`, not `kafkacat`"
        );
        let linux = KCAT.linux.expect("kcat must support Linux");
        assert_eq!(
            linux.apt,
            Some("kafkacat"),
            "Debian / Ubuntu stable still package as `kafkacat`"
        );
        assert_eq!(linux.dnf, Some("kcat"));
        assert_eq!(linux.pacman, Some("kcat"));
        assert_eq!(linux.apk, Some("kcat"));
        assert!(KCAT.windows.is_none(), "no first-party winget manifest");
    }
}
