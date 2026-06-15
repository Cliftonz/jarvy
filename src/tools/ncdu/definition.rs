//! ncdu - NCurses disk usage analyzer
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(NCDU, {
    command: "ncdu",
    macos: { brew: "ncdu" },
    linux: { uniform: "ncdu" },
    bsd: { pkg: "ncdu" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncdu_registration_shape() {
        assert_eq!(NCDU.command, "ncdu");
        let mac = NCDU.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ncdu"));
    }
}
