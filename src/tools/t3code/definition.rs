//! T3 Code - the `t3` command line tool
//!
//! `t3` is the CLI distributed from the pingdotgg/t3code repository.

use crate::define_tool;

define_tool!(T3CODE, {
    command: "t3",
    // No brew formula or winget package exists for the CLI as of 2026-11;
    // the npm fallback route covers all platforms via `t3` (bin = `t3`,
    // verified 2026-11). The winget and cask packages are the desktop app,
    // registered separately as t3code-desktop.
    fallback: { npm: "t3" },
    category: "ai-agent",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t3code_registration_shape() {
        assert_eq!(T3CODE.command, "t3");
        assert!(T3CODE.macos.is_none());
        assert!(T3CODE.linux.is_none());
        assert!(T3CODE.windows.is_none());
        assert_eq!(T3CODE.fallback.len(), 1);
        assert_eq!(T3CODE.fallback[0].eco, crate::tools::spec::FallbackEco::Npm);
        assert_eq!(T3CODE.fallback[0].package, "t3");
        assert_eq!(T3CODE.category, Some("ai-agent"));
    }
}
