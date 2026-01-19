//! java - Java Development Kit
//!
//! Java is a widely-used object-oriented programming language and computing platform.
//! This tool installs the OpenJDK distribution.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(JAVA, {
    command: "java",
    macos: { brew: "openjdk" },
    linux: { apt: "default-jdk", dnf: "java-latest-openjdk", pacman: "jdk-openjdk", apk: "openjdk21" },
    windows: { winget: "Microsoft.OpenJDK.21", choco: "openjdk" },
    bsd: { pkg: "openjdk21" },
    default_hook: {
        description: "Configure JAVA_HOME environment variable",
        script: r#"
# Set JAVA_HOME based on platform
if [ "$(uname)" = "Darwin" ]; then
    JAVA_HOME_PATH="$(/usr/libexec/java_home 2>/dev/null || true)"
elif [ -d "/usr/lib/jvm/default" ]; then
    JAVA_HOME_PATH="/usr/lib/jvm/default"
elif [ -d "/usr/lib/jvm/java-21-openjdk" ]; then
    JAVA_HOME_PATH="/usr/lib/jvm/java-21-openjdk"
fi

if [ -n "$JAVA_HOME_PATH" ]; then
    JAVA_EXPORT="export JAVA_HOME=\"$JAVA_HOME_PATH\""

    # Add to .bashrc if not present
    if [ -f "$HOME/.bashrc" ] && ! grep -q 'JAVA_HOME' "$HOME/.bashrc"; then
        echo "$JAVA_EXPORT" >> "$HOME/.bashrc"
        echo "Added JAVA_HOME to ~/.bashrc"
    fi

    # Add to .zshrc if not present
    if [ -f "$HOME/.zshrc" ] && ! grep -q 'JAVA_HOME' "$HOME/.zshrc"; then
        echo "$JAVA_EXPORT" >> "$HOME/.zshrc"
        echo "Added JAVA_HOME to ~/.zshrc"
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_java_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
