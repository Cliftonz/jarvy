//! deno - a modern runtime for JavaScript and TypeScript
//!
//! Deno is a secure runtime for JavaScript and TypeScript built on V8.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DENO, {
    command: "deno",
    macos: { brew: "deno" },
    linux: { brew: "deno" },
    windows: { winget: "DenoLand.Deno" },
    bsd: { pkg: "deno" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_deno_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
