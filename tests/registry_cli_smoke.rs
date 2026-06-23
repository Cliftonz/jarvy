//! CLI-level smoke tests for `jarvy registry {sync,status,clear}`.
//!
//! Covers Item 30 from the parallel-code-review enhancement plan: pins
//! the exit-code-mapping contract of `registry_cmd::run_registry` and
//! the user-facing stdout/stderr text. Integration tests for the
//! orchestrator itself live in `tests/registry_remote_integration.rs`.

#![allow(unsafe_code)] // env mutation is fenced by #[serial] + cleanup guards

use std::process::Command;

use serial_test::serial;
use tempfile::TempDir;

const EXIT_CONFIG_ERROR: i32 = 2;

/// Per-test JARVY_HOME tempdir + env restoration. Mirrors the helper in
/// registry_remote_integration.rs but doesn't need
/// JARVY_REGISTRY_ALLOW_INSECURE_FETCH because these tests don't make
/// network calls.
struct HomeGuard {
    _tmp: TempDir,
    prev_home: Option<std::ffi::OsString>,
}

impl HomeGuard {
    fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let prev_home = std::env::var_os("JARVY_HOME");
        // SAFETY: serialized by #[serial(registry_cli)].
        unsafe {
            std::env::set_var("JARVY_HOME", tmp.path());
        }
        Self {
            _tmp: tmp,
            prev_home,
        }
    }

    fn path(&self) -> &std::path::Path {
        self._tmp.path()
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        // SAFETY: serialized by #[serial(registry_cli)].
        unsafe {
            match &self.prev_home {
                Some(v) => std::env::set_var("JARVY_HOME", v),
                None => std::env::remove_var("JARVY_HOME"),
            }
        }
    }
}

fn jarvy(home: &HomeGuard) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_HOME", home.path())
        .env("JARVY_TELEMETRY", "0")
        .env("JARVY_TEST_MODE", "1");
    c
}

#[test]
#[serial(registry_cli)]
fn sync_without_config_exits_with_clear_error() {
    let home = HomeGuard::new();
    let output = jarvy(&home)
        .arg("registry")
        .arg("sync")
        .output()
        .expect("spawn");

    assert_eq!(
        output.status.code(),
        Some(EXIT_CONFIG_ERROR),
        "sync without [registry] must exit {EXIT_CONFIG_ERROR}; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("registry not configured"),
        "stderr should explain the missing config; got:\n{stderr}"
    );
}

#[test]
#[serial(registry_cli)]
fn status_without_prior_sync_prints_hint_and_exits_zero() {
    let home = HomeGuard::new();
    let output = jarvy(&home)
        .arg("registry")
        .arg("status")
        .output()
        .expect("spawn");

    assert!(
        output.status.success(),
        "status on empty cache must exit 0; got {:?}, stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No registry sync recorded"),
        "stdout should hint the user to run sync; got:\n{stdout}"
    );
}

#[test]
#[serial(registry_cli)]
fn clear_on_empty_cache_exits_zero_with_message() {
    let home = HomeGuard::new();
    let output = jarvy(&home)
        .arg("registry")
        .arg("clear")
        .output()
        .expect("spawn");

    assert!(
        output.status.success(),
        "clear on empty cache must exit 0; got {:?}, stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("already empty"),
        "stdout should announce no-op; got:\n{stdout}"
    );
}

#[test]
#[serial(registry_cli)]
fn clear_removes_existing_cache_dir() {
    let home = HomeGuard::new();
    // Pre-seed the cache dir so clear has something to do.
    let cache_dir = home.path().join("tools.d").join(".remote");
    std::fs::create_dir_all(cache_dir.join("tools")).expect("create cache");
    std::fs::write(cache_dir.join("meta.json"), r#"{"tools_count":0}"#).expect("write meta");
    assert!(cache_dir.exists());

    let output = jarvy(&home)
        .arg("registry")
        .arg("clear")
        .output()
        .expect("spawn");
    assert!(output.status.success(), "clear must exit 0");
    assert!(
        !cache_dir.exists(),
        "cache dir must be gone after `jarvy registry clear`"
    );
}

#[test]
#[serial(registry_cli)]
fn sync_with_http_url_refuses_at_validate_safety() {
    let home = HomeGuard::new();
    let cfg = r#"
[registry]
url = "http://example.com/r/"
enabled = true
"#;
    std::fs::write(home.path().join("config.toml"), cfg).expect("write config");

    let output = jarvy(&home)
        .arg("registry")
        .arg("sync")
        .output()
        .expect("spawn");
    assert_eq!(
        output.status.code(),
        Some(EXIT_CONFIG_ERROR),
        "non-https url must be refused; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must be https"),
        "stderr should explain the rejection; got:\n{stderr}"
    );
}

#[test]
#[serial(registry_cli)]
fn sync_with_unanchored_regex_refused() {
    let home = HomeGuard::new();
    let cfg = r#"
[registry]
url = "https://example.com/r/"
enabled = true
signature_identity_regexp = "github.com/.*"
"#;
    std::fs::write(home.path().join("config.toml"), cfg).expect("write config");

    let output = jarvy(&home)
        .arg("registry")
        .arg("sync")
        .output()
        .expect("spawn");
    assert_eq!(
        output.status.code(),
        Some(EXIT_CONFIG_ERROR),
        "unanchored regex must be refused; got {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("fully anchored"),
        "stderr should explain anchor requirement; got:\n{stderr}"
    );
}
