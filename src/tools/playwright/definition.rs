//! Playwright - end-to-end testing framework for web apps
//!
//! Playwright drives Chromium, Firefox, and WebKit with a single API.
//! It is distributed on the npm registry, so installation goes through
//! a custom installer that requires Node.js/npm (no brew formula,
//! distro package, or first-party winget manifest exists).
//!
//! Note: browser binaries are NOT downloaded at install time — run
//! `playwright install` afterwards (project CI usually does this) to
//! avoid a surprise multi-hundred-MB download during `jarvy setup`.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};

define_tool!(PLAYWRIGHT, {
    command: "playwright",
    custom_install: install_playwright,
    depends_on_one_of: &["node", "nvm"],
    category: "testing",
});

fn install_playwright(_min_hint: &str) -> Result<(), InstallError> {
    if has("playwright") {
        return Ok(());
    }

    if !has("npm") {
        return Err(InstallError::Prereq(
            "npm not found. Install Node.js (e.g. via nvm) and re-run.",
        ));
    }

    run("npm", &["install", "-g", "playwright"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playwright_registration_shape() {
        assert_eq!(PLAYWRIGHT.command, "playwright");
        assert_eq!(PLAYWRIGHT.category, Some("testing"));
        assert!(
            PLAYWRIGHT.custom_install.is_some(),
            "npm-distributed tool must use a custom installer"
        );
        assert!(PLAYWRIGHT.macos.is_none(), "no brew formula — npm only");
        assert!(
            PLAYWRIGHT.windows.is_none(),
            "no first-party winget manifest — npm only"
        );
    }
}
