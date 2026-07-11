//! todoist - unofficial Todoist CLI client (sachaos/todoist)
//!
//! Binary is `todoist`; the Homebrew formula is `todoist-cli-go` (formerly
//! `todoist`, renamed to free the name for Doist's official CLI). No native
//! apt/dnf/pacman package exists, so Linux installs via Linuxbrew.

use crate::define_tool;

define_tool!(TODOIST, {
    command: "todoist",
    macos: { brew: "todoist-cli-go" },
    // No native apt/dnf/pacman package as of 2026-07; install via Linuxbrew or
    // `go install github.com/sachaos/todoist@latest`. Upstream: https://github.com/sachaos/todoist
    linux: { brew: "todoist-cli-go" },
    // No first-party winget/choco/scoop manifest as of 2026-07; install from
    // https://github.com/sachaos/todoist (Go).
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todoist_registration_shape() {
        assert_eq!(TODOIST.command, "todoist");
        let mac = TODOIST.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("todoist-cli-go"));
        let linux = TODOIST.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("todoist-cli-go"));
        assert!(TODOIST.windows.is_none());
    }
}
