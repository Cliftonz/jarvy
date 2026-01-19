//! scala - multi-paradigm programming language
//!
//! Scala is a programming language combining object-oriented and functional programming.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SCALA, {
    command: "scala",
    macos: { brew: "scala" },
    linux: { apt: "scala", dnf: "scala", pacman: "scala", apk: "scala" },
    windows: { winget: "Scala.Scala.3", choco: "scala" },
    bsd: { pkg: "scala" },
    depends_on: &["java"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_scala_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
