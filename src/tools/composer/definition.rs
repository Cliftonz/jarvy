//! Composer — dependency manager for PHP.
//!
//! Homepage: <https://getcomposer.org>. Standard install: `brew install
//! composer` on macOS, `apt/dnf/pacman install composer` on Linux
//! (the package name is uniform across major distros as of 2026-07).
//!
//! Windows: OMITTED. Composer's canonical Windows installer is
//! `Composer-Setup.exe` from getcomposer.org — there is no first-party
//! `Composer.Composer` winget manifest (only the unrelated
//! `Microsoft.BotFrameworkComposer` publisher namespace exists).
//! Shipping a placeholder ID would create supply-chain exposure per
//! the CLAUDE.md "Omit unsupported platforms" rule; runtime emits
//! `tool.unsupported` and directs the user to <https://getcomposer.org>.

use crate::define_tool;

define_tool!(COMPOSER, {
    command: "composer",
    repo: "composer/composer",
    macos: { brew: "composer" },
    linux: { uniform: "composer" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_registration_shape() {
        assert_eq!(COMPOSER.command, "composer");
        assert_eq!(COMPOSER.macos.unwrap().brew, Some("composer"));
        assert!(
            COMPOSER.windows.is_none(),
            "no first-party winget manifest as of 2026-07 — see module docs"
        );
    }
}
