//! tree - directory listing in tree format
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: On Windows, tree is built-in, so we don't need to install it.

use crate::define_tool;

define_tool!(TREE, {
    command: "tree",
    macos: { brew: "tree" },
    linux: { uniform: "tree" },
    // Windows has built-in tree.exe, no package needed
    bsd: { pkg: "tree" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_registration_shape() {
        assert_eq!(TREE.command, "tree");
        let mac = TREE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("tree"));
    }
}
