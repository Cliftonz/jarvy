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
    linux: { uniform: "kaf" },
    // No first-party winget manifest; left None on Windows.
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kaf_registration_shape() {
        assert_eq!(KAF.command, "kaf");
        let mac = KAF.macos.expect("kaf must support macOS");
        assert_eq!(mac.brew, Some("kaf"));
    }
}
