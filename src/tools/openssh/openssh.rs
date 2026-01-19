//! openssh - OpenSSH client and server
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(OPENSSH, {
    command: "ssh",
    macos: { brew: "openssh" },
    linux: { uniform: "openssh" },
    windows: { winget: "Microsoft.OpenSSH.Beta" },
    bsd: { pkg: "openssh-portable" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_openssh_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
