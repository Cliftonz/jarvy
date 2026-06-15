//! ocaml - industrial-strength functional programming language
//!
//! OCaml is an industrial-strength programming language supporting functional,
//! imperative and object-oriented styles. It emphasizes expressiveness and
//! safety.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(OCAML, {
    command: "ocaml",
    macos: { brew: "ocaml" },
    linux: { apt: "ocaml", dnf: "ocaml", pacman: "ocaml", apk: "ocaml" },
    windows: { winget: "OCaml.OCaml" },
    bsd: { pkg: "ocaml" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocaml_registration_shape() {
        assert_eq!(OCAML.command, "ocaml");
        let mac = OCAML.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ocaml"));
        let win = OCAML.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("OCaml.OCaml"));
    }
}
