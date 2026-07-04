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
    repo: "ansible/molecule",
    macos: { brew: "molecule" },
    linux: { apt: "molecule", dnf: "molecule", pacman: "molecule", apk: "molecule" },
    bsd: { pkg: "molecule" },
    depends_on: &["ansible"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn molecule_registration_shape() {
        assert_eq!(MOLECULE.command, "molecule");
        let mac = MOLECULE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("molecule"));
    }
}
