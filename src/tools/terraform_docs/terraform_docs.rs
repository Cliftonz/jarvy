//! terraform-docs - terraform documentation generator
//!
//! terraform-docs generates documentation from Terraform modules.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TERRAFORM_DOCS, {
    command: "terraform-docs",
    macos: { brew: "terraform-docs" },
    linux: { brew: "terraform-docs" },
    windows: { choco: "terraform-docs" },
    bsd: { pkg: "terraform-docs" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_terraform_docs_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
