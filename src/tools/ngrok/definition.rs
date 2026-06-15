//! ngrok - secure tunneling to localhost
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NGROK, {
    command: "ngrok",
    macos: { brew: "ngrok" },
    linux: { uniform: "ngrok" },
    windows: { winget: "Ngrok.Ngrok" },
    bsd: { pkg: "ngrok" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ngrok_registration_shape() {
        assert_eq!(NGROK.command, "ngrok");
        let mac = NGROK.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ngrok"));
        let win = NGROK.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Ngrok.Ngrok"));
    }
}
