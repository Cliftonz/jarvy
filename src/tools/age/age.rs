//! age - Simple, modern, and secure file encryption
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AGE, {
    command: "age",
    macos: { brew: "age" },
    linux: { uniform: "age" },
    windows: { winget: "FiloSottile.age" },
    bsd: { pkg: "age" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_age_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
