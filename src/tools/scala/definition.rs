//! scala - multi-paradigm programming language
//!
//! Scala is a programming language combining object-oriented and functional programming.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SCALA, {
    command: "scala",
    repo: "scala/scala",
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
    fn scala_registration_shape() {
        assert_eq!(SCALA.command, "scala");
        let mac = SCALA.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("scala"));
        let win = SCALA.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Scala.Scala.3"));
    }
}
