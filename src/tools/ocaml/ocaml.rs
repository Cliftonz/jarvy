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
    fn ensure_ocaml_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
