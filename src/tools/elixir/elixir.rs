//! elixir - Elixir programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ELIXIR, {
    command: "elixir",
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
    fn ensure_elixir_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
