//! erlang - Erlang programming language
//!
//! Erlang is a programming language used to build massively scalable soft real-time systems
//! with requirements on high availability. It is the runtime for Elixir.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ERLANG, {
    command: "erl",
    macos: { brew: "erlang" },
    linux: { apt: "erlang", dnf: "erlang", pacman: "erlang", apk: "erlang" },
    windows: { winget: "Erlang.ErlangOTP", choco: "erlang" },
    bsd: { pkg: "erlang" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erlang_registration_shape() {
        assert_eq!(ERLANG.command, "erl");
        let mac = ERLANG.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("erlang"));
        let win = ERLANG.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Erlang.ErlangOTP"));
    }
}
