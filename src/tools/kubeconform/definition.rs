//! kubeconform - Kubernetes manifest validator
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUBECONFORM, {
    command: "kubeconform",
    macos: { brew: "kubeconform" },
    linux: { brew: "kubeconform" },
    windows: { winget: "YannHamon.kubeconform" },
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubeconform_registration_shape() {
        assert_eq!(KUBECONFORM.command, "kubeconform");

        let macos = KUBECONFORM.macos.expect("must support macOS");
        assert_eq!(macos.brew, Some("kubeconform"));

        let linux = KUBECONFORM.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("kubeconform"));

        let windows = KUBECONFORM.windows.expect("must support Windows");
        assert_eq!(windows.winget, Some("YannHamon.kubeconform"));
    }
}
