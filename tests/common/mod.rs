use std::process::Command;

/// Create a jarvy Command with test mode enabled.
///
/// Sets `JARVY_TEST_MODE=1` to disable interactive prompts.
pub fn jarvy_cmd() -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c
}
