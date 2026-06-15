//! ansible - automation platform
//!
//! Ansible is an IT automation tool for configuration management,
//! application deployment, and task automation.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ANSIBLE, {
    command: "ansible",
    macos: { brew: "ansible" },
    linux: { apt: "ansible", dnf: "ansible", pacman: "ansible", apk: "ansible" },
    windows: { choco: "ansible" },
    bsd: { pkg: "py39-ansible" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansible_registration_shape() {
        assert_eq!(ANSIBLE.command, "ansible");
        let mac = ANSIBLE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ansible"));
    }
}
