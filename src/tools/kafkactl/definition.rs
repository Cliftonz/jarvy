//! kafkactl - declarative Kafka admin CLI (deviceinsight)
//!
//! `kafkactl` is a `kubectl`-shaped CLI for Kafka — declarative
//! `kafkactl apply -f topic.yaml`, `kafkactl get topics`,
//! `kafkactl describe consumer-group`. Often preferred over `kcat`
//! when admin work dominates over message debugging.

use crate::define_tool;

define_tool!(KAFKACTL, {
    command: "kafkactl",
    macos: { brew: "deviceinsight/packages/kafkactl" },
    // Linux: install via Linuxbrew through the same tap, or use the
    // upstream release binary. No native distro package.
    linux: { brew: "deviceinsight/packages/kafkactl" },
    // No first-party winget manifest; the go fallback route covers
    // Windows — upstream README documents
    // `go install github.com/deviceinsight/kafkactl/v5@latest`
    // (verified 2026-08, no replace directives in go.mod).
    fallback: { go: "github.com/deviceinsight/kafkactl/v5" },
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kafkactl_registration_shape() {
        assert_eq!(KAFKACTL.command, "kafkactl");
        assert_eq!(KAFKACTL.category, Some("messaging"));
        let mac = KAFKACTL.macos.expect("kafkactl must support macOS");
        assert_eq!(
            mac.brew,
            Some("deviceinsight/packages/kafkactl"),
            "macOS formula lives in the deviceinsight/packages tap (verify upstream if this fails — formula may have promoted to homebrew-core)"
        );
        let linux = KAFKACTL.linux.expect("kafkactl must support Linux");
        assert_eq!(linux.brew, Some("deviceinsight/packages/kafkactl"));
        assert!(KAFKACTL.windows.is_none(), "no first-party winget manifest");
        assert_eq!(KAFKACTL.fallback.len(), 1);
        assert_eq!(
            KAFKACTL.fallback[0].eco,
            crate::tools::spec::FallbackEco::Go
        );
        assert_eq!(
            KAFKACTL.fallback[0].package,
            "github.com/deviceinsight/kafkactl/v5"
        );
    }
}
