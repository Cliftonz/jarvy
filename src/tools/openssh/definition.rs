//! openssh - OpenSSH client and server
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(OPENSSH, {
    command: "ssh",
    repo: "openssh/openssh-portable",
    macos: { brew: "openssh" },
    linux: { uniform: "openssh" },
    windows: { winget: "Microsoft.OpenSSH.Beta" },
    bsd: { pkg: "openssh-portable" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openssh_registration_shape() {
        assert_eq!(OPENSSH.command, "ssh");
        let mac = OPENSSH.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("openssh"));
        let win = OPENSSH.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Microsoft.OpenSSH.Beta"));
    }
}
