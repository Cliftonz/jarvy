//! nvim - Neovim editor
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NVIM, {
    command: "nvim",
    macos: { brew: "neovim" },
    linux: { uniform: "neovim" },
    windows: { winget: "Neovim.Neovim" },
    bsd: { pkg: "neovim" },
    default_hook: {
        description: "Create ~/.config/nvim with a starter init.lua when no config exists",
        script: r#"
# Never touch an existing config — starter file only on a clean machine
NVIM_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/nvim"
if [ ! -e "$NVIM_DIR/init.lua" ] && [ ! -e "$NVIM_DIR/init.vim" ]; then
    mkdir -p "$NVIM_DIR"
    cat > "$NVIM_DIR/init.lua" <<'EOF'
-- Starter config created by jarvy (safe to replace).
vim.opt.number = true
vim.opt.expandtab = true
vim.opt.shiftwidth = 4
vim.opt.tabstop = 4
vim.opt.smartindent = true
vim.opt.termguicolors = true
EOF
    echo "Created starter config at $NVIM_DIR/init.lua"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvim_registration_shape() {
        assert_eq!(NVIM.command, "nvim");
        let mac = NVIM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("neovim"));
        let win = NVIM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Neovim.Neovim"));
    }
}
