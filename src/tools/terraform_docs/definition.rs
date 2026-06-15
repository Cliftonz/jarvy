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
    fn terraform_docs_registration_shape() {
        assert_eq!(TERRAFORM_DOCS.command, "terraform-docs");
        let mac = TERRAFORM_DOCS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("terraform-docs"));
    }
}
