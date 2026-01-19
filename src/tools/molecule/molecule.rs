//! molecule - Ansible role testing framework
//!
//! Molecule aids in the development and testing of Ansible roles.
//! It provides support for testing with multiple instances, operating
//! systems, and test frameworks.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MOLECULE, {
    command: "molecule",
    macos: { brew: "molecule" },
    linux: { apt: "molecule", dnf: "molecule", pacman: "molecule", apk: "molecule" },
    bsd: { pkg: "molecule" },
    depends_on: &["ansible"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_molecule_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
