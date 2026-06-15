//! vault - HashiCorp secrets management
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(VAULT, {
    command: "vault",
    macos: { brew: "vault" },
    linux: { uniform: "vault" },
    windows: { winget: "HashiCorp.Vault" },
    bsd: { pkg: "vault" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_registration_shape() {
        assert_eq!(VAULT.command, "vault");
        let mac = VAULT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("vault"));
        let win = VAULT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("HashiCorp.Vault"));
    }
}
