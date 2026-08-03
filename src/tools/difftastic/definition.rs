//! difftastic — structural diff tool that compares files by syntax tree
//! rather than by line, so refactors read as what actually changed.
//!
//! The installed binary is `difft` (package name is `difftastic`), so
//! presence detection probes `difft`.

use crate::define_tool;

define_tool!(DIFFTASTIC, {
    command: "difft",
    macos: { brew: "difftastic" },
    linux: { brew: "difftastic" },
    windows: { winget: "Wilfred.Difftastic" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difftastic_registration_shape() {
        assert_eq!(DIFFTASTIC.command, "difft");
        assert_eq!(DIFFTASTIC.macos.expect("macOS").brew, Some("difftastic"));
        assert_eq!(DIFFTASTIC.linux.expect("Linux").brew, Some("difftastic"));
        assert_eq!(
            DIFFTASTIC.windows.expect("Windows").winget,
            Some("Wilfred.Difftastic")
        );
        assert!(DIFFTASTIC.depends_on.is_none());
    }
}
