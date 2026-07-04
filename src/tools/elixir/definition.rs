//! elixir - Elixir programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ELIXIR, {
    command: "elixir",
    repo: "elixir-lang/elixir",
    macos: { brew: "elixir" },
    linux: { uniform: "elixir" },
    windows: { winget: "Elixir.Elixir" },
    bsd: { pkg: "elixir" },
    depends_on: &["erlang"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elixir_registration_shape() {
        assert_eq!(ELIXIR.command, "elixir");
        let mac = ELIXIR.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("elixir"));
        let win = ELIXIR.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Elixir.Elixir"));
    }
}
