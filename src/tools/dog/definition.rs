//! dog - command-line DNS client
//!
//! Dog is a command-line DNS client, like dig. It has colorful output,
//! understands normal command-line argument syntax, supports the DNS-over-TLS
//! and DNS-over-HTTPS protocols, and can emit JSON.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DOG, {
    command: "dog",
    repo: "ogham/dog",
    macos: { brew: "dog" },
    linux: { apt: "dog", dnf: "dog", pacman: "dog", apk: "dog" },
    bsd: { pkg: "dog" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dog_registration_shape() {
        assert_eq!(DOG.command, "dog");
        let mac = DOG.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("dog"));
    }
}
