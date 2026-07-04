//! terraform - infrastructure as code tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TERRAFORM, {
    command: "terraform",
    repo: "hashicorp/terraform",
    macos: { brew: "terraform" },
    linux: { uniform: "terraform" },
    windows: { winget: "HashiCorp.Terraform" },
    bsd: { pkg: "terraform" },
    default_hook: {
        description: "Install Terraform shell autocomplete",
        script: r#"
# Terraform shell autocomplete
# The -install-autocomplete command is idempotent

# Only run if terraform is available
if command -v terraform >/dev/null 2>&1; then
    # terraform -install-autocomplete modifies shell rc files
    # It's idempotent so safe to run multiple times
    terraform -install-autocomplete 2>/dev/null || true
    echo "Terraform autocomplete installed (restart shell to activate)"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terraform_registration_shape() {
        assert_eq!(TERRAFORM.command, "terraform");
        let mac = TERRAFORM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("terraform"));
        let win = TERRAFORM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("HashiCorp.Terraform"));
    }
}
