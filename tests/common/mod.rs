// Each integration test compiles `common` as a fresh module; items not
// used by a given test file produce dead_code warnings that are not
// actionable. This is the conventional pattern for tests/common/.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Create a jarvy Command with test mode enabled.
///
/// Sets `JARVY_TEST_MODE=1` to disable interactive prompts.
pub fn jarvy_cmd() -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c
}

/// Create a jarvy Command in fast-test mode: `JARVY_TEST_MODE=1` plus
/// `JARVY_FAST_TEST=1`. Use for integration tests that exercise the CLI
/// surface but should skip external command execution (network, package
/// managers, subprocess installs). Adopted by `examples_validation.rs`
/// and `new_commands.rs`.
pub fn jarvy_fast_cmd() -> Command {
    let mut c = jarvy_cmd();
    c.env("JARVY_FAST_TEST", "1");
    c
}

// ---------------------------------------------------------------------
// Container-test helpers shared by `sandbox_integration.rs` and
// `e2e_install_pipeline.rs`. The two test families have divergent
// `skip_reason`/`container_request` bodies (intentionally — see PRD-053
// review F2/F3), but these three pure functions and the magic-path
// constants are byte-identical between them and live here to prevent
// drift.
// ---------------------------------------------------------------------

/// Path inside containers where the host jarvy is bind-mounted.
pub const CONTAINER_JARVY_BIN: &str = "/usr/local/bin/jarvy";

/// Writable home directory inside containers. Jarvy's log/probe writers
/// need a writable `$HOME` and `$JARVY_HOME`; the container's root fs
/// stays read-only in hardened tests, so we point both at `/tmp`.
pub const CONTAINER_HOME: &str = "/tmp";
pub const CONTAINER_JARVY_HOME: &str = "/tmp/.jarvy";

/// Container sleep duration. Long enough for image-pull + smoke exec
/// on a cold runner; short enough that a hung harness doesn't park a
/// runner for an hour.
pub const CONTAINER_LIFETIME_SECS: &str = "120";

/// Returns true when the docker daemon answers `docker info`. Cached
/// across the test run — the answer doesn't change mid-`cargo test`.
pub fn docker_available() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        Command::new("docker")
            .arg("info")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Resolve the jarvy binary to mount into containers.
///
/// Order: `JARVY_TEST_BIN` env override (cross-built linux binary on
/// macOS), then Cargo's `CARGO_BIN_EXE_jarvy` (the just-built host
/// binary on Linux CI).
pub fn host_jarvy_path() -> PathBuf {
    std::env::var("JARVY_TEST_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_BIN_EXE_jarvy")))
}

/// True if the resolved binary starts with the ELF magic `0x7F ELF`.
/// Skipping when it isn't avoids exec'ing mach-o / PE binaries inside
/// linux containers and lighting up the suite in red.
pub fn is_linux_elf(path: &Path) -> bool {
    use std::io::Read;
    let mut buf = [0u8; 4];
    match std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut buf)) {
        Ok(()) => buf == [0x7f, b'E', b'L', b'F'],
        Err(_) => false,
    }
}

/// First 12 chars of the SHA-256 of the file at `path`. Lets failure
/// messages identify *which* jarvy binary was mounted — useful when
/// `JARVY_TEST_BIN` points at a stale cross-build dir.
pub fn short_sha256(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return "unknown".into(),
    };
    hasher.update(&bytes);
    let full = hasher.finalize();
    hex::encode(&full[..6])
}

/// Replace anything looking like a credential pair inside a captured
/// container stdout/stderr blob before splicing it into a panic
/// message. Conservative — only matches obvious `token=...`,
/// `password=...`, `KEY=value`-shaped pairs. Capped output length
/// bounds the blast radius if a hostile mounted binary dumps
/// `/proc/self/environ`.
pub fn scrub_for_panic(s: &str) -> String {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::RegexBuilder::new(
            r"(?P<key>(?:token|secret|password|passwd|api[_-]?key|auth|bearer)\w*)\s*[=:]\s*\S+",
        )
        .case_insensitive(true)
        .build()
        .expect("regex compiles")
    });
    let scrubbed = re.replace_all(s, "$key=[REDACTED]").into_owned();
    const MAX: usize = 4096;
    if scrubbed.len() > MAX {
        let mut s = scrubbed;
        s.truncate(MAX);
        s.push_str("\n...[truncated]");
        s
    } else {
        scrubbed
    }
}
