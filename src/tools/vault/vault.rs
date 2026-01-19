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
    fn ensure_vault_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
