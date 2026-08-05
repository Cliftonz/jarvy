//! kaf - interactive Kafka CLI (alternative to kcat)
//!
//! `kaf` is a Go-based Kafka CLI by @birdayz with a friendlier UX
//! than `kcat` — multi-cluster context switching (`kaf config use
//! prod`), schema-aware output, and convenient consumer-group
//! management. Common pairing in shops that have multiple Kafka
//! clusters to admin.

use crate::define_tool;

define_tool!(KAF, {
    command: "kaf",
    macos: { brew: "kaf" },
    // Linux: no distro package; install via Linuxbrew or release binary.
    linux: { brew: "kaf" },
    // No first-party winget manifest; the go fallback route covers
    // Windows (verified 2026-08).
    fallback: { go: "github.com/birdayz/kaf/cmd/kaf" },
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kaf_registration_shape() {
        assert_eq!(KAF.command, "kaf");
        assert_eq!(KAF.category, Some("messaging"));
        let mac = KAF.macos.expect("kaf must support macOS");
        assert_eq!(mac.brew, Some("kaf"));
        let linux = KAF.linux.expect("kaf must support Linux");
        assert_eq!(linux.brew, Some("kaf"));
        assert!(KAF.windows.is_none(), "no first-party winget manifest");
        assert_eq!(KAF.fallback.len(), 1);
        assert_eq!(KAF.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(KAF.fallback[0].package, "github.com/birdayz/kaf/cmd/kaf");
    }
}
