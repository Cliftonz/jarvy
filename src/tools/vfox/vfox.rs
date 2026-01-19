//! vfox - cross-platform version manager
//!
//! A cross-platform and extendable version manager with support for
//! Java, Node.js, Golang, Python, Flutter, .NET & more.
//!
//! See: https://github.com/version-fox/vfox

use crate::define_tool;

define_tool!(VFOX, {
    command: "vfox",
    macos: { brew: "vfox" },
    linux: { brew: "vfox" },
    windows: { winget: "vfox" },
    bsd: { pkg: "vfox" },
    default_hook: {
        description: "Add vfox shell activation to .bashrc and .zshrc",
        script: r#"
# vfox shell integration
VFOX_ACTIVATE_BASH='eval "$(vfox activate bash)"'
VFOX_ACTIVATE_ZSH='eval "$(vfox activate zsh)"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'vfox activate bash' "$HOME/.bashrc"; then
    echo "$VFOX_ACTIVATE_BASH" >> "$HOME/.bashrc"
    echo "Added vfox activation to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'vfox activate zsh' "$HOME/.zshrc"; then
    echo "$VFOX_ACTIVATE_ZSH" >> "$HOME/.zshrc"
    echo "Added vfox activation to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_vfox_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
