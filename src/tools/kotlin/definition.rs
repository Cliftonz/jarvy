//! kotlin - statically typed programming language
//!
//! Kotlin is a modern programming language that targets the JVM.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KOTLIN, {
    command: "kotlin",
    macos: { brew: "kotlin" },
    linux: { apt: "kotlin", dnf: "kotlin", pacman: "kotlin", apk: "kotlin" },
    windows: { winget: "JetBrains.Kotlin.Compiler", choco: "kotlinc" },
    bsd: { pkg: "kotlin" },
    depends_on: &["java"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kotlin_registration_shape() {
        assert_eq!(KOTLIN.command, "kotlin");
        let mac = KOTLIN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kotlin"));
        let win = KOTLIN.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("JetBrains.Kotlin.Compiler"));
    }
}
