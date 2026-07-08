//! Cypress - JavaScript end-to-end testing framework
//!
//! Cypress runs component and end-to-end tests in a real browser with
//! time-travel debugging. It is distributed on the npm registry, so
//! installation goes through a custom installer that requires
//! Node.js/npm (no brew formula, distro package, or first-party
//! winget manifest exists — the npm postinstall step downloads the
//! Cypress binary itself).

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};

define_tool!(CYPRESS, {
    command: "cypress",
    custom_install: install_cypress,
    depends_on_one_of: &["node", "nvm"],
    category: "testing",
});

fn install_cypress(_min_hint: &str) -> Result<(), InstallError> {
    if has("cypress") {
        return Ok(());
    }

    if !has("npm") {
        return Err(InstallError::Prereq(
            "npm not found. Install Node.js (e.g. via nvm) and re-run.",
        ));
    }

    run("npm", &["install", "-g", "cypress"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cypress_registration_shape() {
        assert_eq!(CYPRESS.command, "cypress");
        assert_eq!(CYPRESS.category, Some("testing"));
        assert!(
            CYPRESS.custom_install.is_some(),
            "npm-distributed tool must use a custom installer"
        );
        assert!(CYPRESS.macos.is_none(), "no brew formula — npm only");
        assert!(
            CYPRESS.windows.is_none(),
            "no first-party winget manifest — npm only"
        );
    }
}
