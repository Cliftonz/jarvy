//! uv - extremely fast Python package and project manager
//!
//! Astral's all-in-one Python toolchain: a drop-in replacement for `pip`,
//! `pip-tools`, `pipx`, `poetry`, `pyenv`, and `virtualenv`.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(UV, {
    command: "uv",
    macos: { brew: "uv" },
    linux: { brew: "uv", apk: "uv" },
    windows: { winget: "astral-sh.uv" },
    bsd: { pkg: "uv" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_uv_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
