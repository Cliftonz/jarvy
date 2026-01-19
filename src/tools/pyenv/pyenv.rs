//! pyenv - python version manager
//!
//! pyenv lets you easily switch between multiple versions of Python.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PYENV, {
    command: "pyenv",
    macos: { brew: "pyenv" },
    linux: { brew: "pyenv" },
    bsd: { pkg: "pyenv" },
    default_hook: {
        description: "Add pyenv shell initialization to .bashrc and .zshrc",
        script: r#"
# pyenv shell integration
PYENV_ROOT="$HOME/.pyenv"
PYENV_INIT_BASH='export PYENV_ROOT="$HOME/.pyenv"
[[ -d $PYENV_ROOT/bin ]] && export PATH="$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init - bash)"'

PYENV_INIT_ZSH='export PYENV_ROOT="$HOME/.pyenv"
[[ -d $PYENV_ROOT/bin ]] && export PATH="$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init - zsh)"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'pyenv init' "$HOME/.bashrc"; then
    echo "$PYENV_INIT_BASH" >> "$HOME/.bashrc"
    echo "Added pyenv init to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'pyenv init' "$HOME/.zshrc"; then
    echo "$PYENV_INIT_ZSH" >> "$HOME/.zshrc"
    echo "Added pyenv init to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_pyenv_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
