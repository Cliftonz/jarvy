//! Shared test infrastructure for the `registry_remote` subsystem.
//!
//! Three building blocks the registry test suites use:
//!
//! - [`TestEnv`] — per-test tempdir + env var restoration (`JARVY_HOME`,
//!   `JARVY_REGISTRY_ALLOW_INSECURE_FETCH`, `PATH` for fake-cosign).
//! - [`MockRegistry`] — programmable HTTP/1.1 server bound to a random
//!   loopback port. Tests describe routes + failure modes; the server
//!   serves canned responses.
//! - [`FakeCosign`] — writes a tiny shell script that pretends to be
//!   `cosign verify-blob`. Test code prepends its dir to `PATH` so the
//!   sync orchestrator's cosign subprocess invocation hits our shim.
//!
//! Cross-test conventions:
//! - All tests using these helpers should carry `#[serial(registry_env)]`
//!   from `serial_test` so concurrent tests don't trample each other's
//!   process-wide env mutations.
//! - `TestEnv::drop` restores ALL the env vars it touched; tests that
//!   forget to hold the guard for the full assertion window will see
//!   the next test's env state.

#![allow(dead_code, unsafe_code)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, atomic::AtomicBool};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tempfile::TempDir;

// ===== TestEnv: tempdir + env-var RAII guard =====

/// Per-test sandbox. Holds the tempdir + a snapshot of env vars we
/// mutate so Drop restores prior state.
///
/// Usage:
/// ```ignore
/// #[test]
/// #[serial_test::serial(registry_env)]
/// fn my_test() {
///     let env = TestEnv::new();
///     // env.home() is the tempdir Jarvy will read/write to
///     // env.set_path_prepend(...) for fake-cosign on PATH
/// }
/// ```
pub struct TestEnv {
    tmp: TempDir,
    /// Snapshot of env vars at construction time. Drop restores any we
    /// touched. Vars not mentioned here are left alone.
    snapshots: Vec<(String, Option<OsString>)>,
}

impl TestEnv {
    pub fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir create");
        let mut env = Self {
            tmp,
            snapshots: Vec::new(),
        };
        // Standard registry-test env: pin JARVY_HOME and enable the
        // loopback HTTPS bypass (real TLS is too heavy for unit tests).
        env.set("JARVY_HOME", env.tmp.path().as_os_str().to_os_string());
        env.set("JARVY_REGISTRY_ALLOW_INSECURE_FETCH", "1".into());
        // Disable telemetry so tests don't try to ship events anywhere.
        env.set("JARVY_TELEMETRY", "0".into());
        env.set("JARVY_TEST_MODE", "1".into());
        env
    }

    pub fn home(&self) -> &Path {
        self.tmp.path()
    }

    pub fn config_path(&self) -> PathBuf {
        self.tmp.path().join("config.toml")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.tmp.path().join("tools.d").join(".remote")
    }

    pub fn tools_dir(&self) -> PathBuf {
        self.cache_dir().join("tools")
    }

    pub fn set(&mut self, key: &str, value: OsString) {
        if !self.snapshots.iter().any(|(k, _)| k == key) {
            let prior = std::env::var_os(key);
            self.snapshots.push((key.to_string(), prior));
        }
        // SAFETY: tests using TestEnv carry #[serial(registry_env)] so
        // no other thread is reading or writing these vars concurrently.
        unsafe { std::env::set_var(key, &value) };
    }

    pub fn remove(&mut self, key: &str) {
        if !self.snapshots.iter().any(|(k, _)| k == key) {
            let prior = std::env::var_os(key);
            self.snapshots.push((key.to_string(), prior));
        }
        // SAFETY: see set().
        unsafe { std::env::remove_var(key) };
    }

    /// Prepend `dir` to PATH so spawned child binaries (and our
    /// in-process cosign invocations) hit our test shims first.
    /// Snapshots and restores the original PATH on Drop.
    pub fn prepend_path(&mut self, dir: &Path) {
        let new_path = match std::env::var_os("PATH") {
            Some(p) => {
                let mut new = OsString::from(dir);
                new.push(if cfg!(windows) { ";" } else { ":" });
                new.push(&p);
                new
            }
            None => OsString::from(dir),
        };
        self.set("PATH", new_path);
    }

    /// Write a `[registry]` section into the per-test
    /// `$JARVY_HOME/config.toml`. Convenience for E2E tests.
    pub fn write_registry_config(&self, url: &str, require_signature: bool) {
        let body = format!(
            r#"
[registry]
url = "{url}"
enabled = true
require_signature = {require_signature}
"#
        );
        std::fs::write(self.config_path(), body).expect("write config.toml");
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        // Restore vars in reverse order so prepended PATH unwinds before
        // any var that depended on it (none today, but safe shape).
        for (key, prior) in self.snapshots.drain(..).rev() {
            // SAFETY: serial test gate ensures no concurrent reader.
            unsafe {
                match prior {
                    Some(v) => std::env::set_var(&key, &v),
                    None => std::env::remove_var(&key),
                }
            }
        }
    }
}

// ===== MockRegistry: programmable HTTP/1.1 server =====

/// A canned HTTP response. Construct via `Canned::ok` /
/// `Canned::not_found` / `Canned::redirect` / `Canned::raw` for the
/// common cases, or via the struct literal for arbitrary status/headers.
#[derive(Clone)]
pub struct Canned {
    pub status: u16,
    pub status_text: &'static str,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
    /// If set, server sleeps this long before responding. Models a slow
    /// upstream — combined with a sub-second test timeout this exercises
    /// the response-timeout path.
    pub delay: Option<Duration>,
    /// If set, server closes the connection after writing only this many
    /// bytes (truncates the body mid-stream). Models a network drop.
    pub truncate_after: Option<usize>,
}

impl Canned {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            status_text: "OK",
            body: body.into(),
            headers: Vec::new(),
            delay: None,
            truncate_after: None,
        }
    }
    pub fn not_found() -> Self {
        Self {
            status: 404,
            status_text: "Not Found",
            body: b"not found".to_vec(),
            headers: Vec::new(),
            delay: None,
            truncate_after: None,
        }
    }
    pub fn server_error() -> Self {
        Self {
            status: 500,
            status_text: "Internal Server Error",
            body: b"oops".to_vec(),
            headers: Vec::new(),
            delay: None,
            truncate_after: None,
        }
    }
    pub fn redirect(to: impl Into<String>) -> Self {
        let to = to.into();
        Self {
            status: 301,
            status_text: "Moved Permanently",
            body: Vec::new(),
            headers: vec![("Location".to_string(), to)],
            delay: None,
            truncate_after: None,
        }
    }
    pub fn delayed(mut self, d: Duration) -> Self {
        self.delay = Some(d);
        self
    }
    pub fn truncated(mut self, after_bytes: usize) -> Self {
        self.truncate_after = Some(after_bytes);
        self
    }
}

/// Handle to a running mock registry server. Drop stops the server
/// via a sentinel self-connect so the blocking accept thread exits
/// promptly (no sleep-poll latency on shutdown).
pub struct MockRegistry {
    pub base_url: String,
    pub port: u16,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    /// Per-path hit counter. The outer `Mutex` serializes
    /// insert-on-first-hit; under contention the hot path is just an
    /// inner `usize` increment.
    pub hits: Arc<Mutex<HashMap<String, usize>>>,
}

impl MockRegistry {
    /// Start a server that serves `routes`. Port is OS-assigned to allow
    /// parallel tests. The accept loop blocks on `accept()` — shutdown
    /// is via a sentinel self-connect from `MockRegistry::shutdown`, so
    /// teardown latency is one TCP roundtrip on localhost, not a 10 ms
    /// poll tick.
    pub fn start(routes: HashMap<String, Canned>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}/");

        let stop = Arc::new(AtomicBool::new(false));
        let stop_srv = Arc::clone(&stop);
        let routes = Arc::new(routes);
        let hits: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));
        let hits_srv = Arc::clone(&hits);

        let handle = thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // Shutdown sentinel: shutdown() connects once
                        // to unblock accept; the resulting stream is a
                        // dummy that the handler should not parse.
                        if stop_srv.load(std::sync::atomic::Ordering::Acquire) {
                            return;
                        }
                        let routes = Arc::clone(&routes);
                        let hits = Arc::clone(&hits_srv);
                        // Smaller stack — handler does a tiny read/write
                        // and exits. Default 2 MiB stack × N parallel
                        // connections wastes address space on CI.
                        let _ = thread::Builder::new()
                            .stack_size(64 * 1024)
                            .spawn(move || handle_request(stream, &routes, &hits));
                    }
                    Err(_) => return,
                }
            }
        });

        MockRegistry {
            base_url,
            port,
            stop,
            handle: Some(handle),
            hits,
        }
    }

    /// Hit count for a path. 0 if the path was never requested.
    pub fn hits_for(&self, path: &str) -> usize {
        self.hits.lock().unwrap().get(path).copied().unwrap_or(0)
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        // Wake the blocking accept() with a sentinel connection. The
        // accept loop sees the stop flag set on its next iteration and
        // returns immediately.
        let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for MockRegistry {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Static fallback for unmatched routes. Allocated once per process
/// instead of rebuilding `b"not found".to_vec()` on every 404.
static NOT_FOUND: OnceLock<Canned> = OnceLock::new();

fn not_found_ref() -> &'static Canned {
    NOT_FOUND.get_or_init(Canned::not_found)
}

fn handle_request(
    mut stream: TcpStream,
    routes: &HashMap<String, Canned>,
    hits: &Arc<Mutex<HashMap<String, usize>>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    // Read until the request headers terminate at b"\r\n\r\n". HTTP/1.1
    // requests usually fit in one packet but partial reads are legal
    // and were a flake vector under cross-binary parallelism.
    let mut buf = Vec::with_capacity(1024);
    let mut tmp = [0u8; 1024];
    loop {
        let n = match stream.read(&mut tmp) {
            Ok(0) => return,
            Ok(n) => n,
            Err(_) => return,
        };
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 65536 {
            break;
        }
    }
    // The request line is ASCII per RFC 7230; parse without allocating.
    let req_str = match std::str::from_utf8(&buf) {
        Ok(s) => s,
        Err(_) => return,
    };
    let path = req_str.split_whitespace().nth(1).unwrap_or("/");

    {
        let mut h = hits.lock().unwrap();
        *h.entry(path.to_string()).or_insert(0) += 1;
    }

    // Borrow the canned response from the Arc'd route table; no per-
    // request clone of (potentially multi-MiB) bodies.
    let canned: &Canned = routes.get(path).unwrap_or_else(|| not_found_ref());

    if let Some(d) = canned.delay {
        thread::sleep(d);
    }

    use std::fmt::Write as _;
    let mut response = String::with_capacity(128);
    let _ = write!(
        response,
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        canned.status,
        canned.status_text,
        canned.body.len()
    );
    for (k, v) in &canned.headers {
        let _ = write!(response, "{k}: {v}\r\n");
    }
    response.push_str("\r\n");
    let _ = stream.write_all(response.as_bytes());

    let body_to_send = match canned.truncate_after {
        Some(n) if n < canned.body.len() => &canned.body[..n],
        _ => &canned.body[..],
    };
    let _ = stream.write_all(body_to_send);
    // If truncated, drop the stream without writing the rest — the
    // client sees a connection-closed mid-body, which the Read variant
    // of FetchError surfaces.
}

// ===== Fixtures: manifest + tool TOML construction =====

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Build a minimal valid tool TOML body for a given name. Always
/// brew-formula-shape — the registry doesn't care about platform
/// content for sync tests.
pub fn tool_toml(name: &str) -> Vec<u8> {
    format!(
        r#"name = "{name}"
command = "{name}"

[macos]
brew = "{name}"
"#
    )
    .into_bytes()
}

/// Build a manifest JSON body listing N tools. Each tool entry's
/// `path = "tools/{name}.toml"` and `sha256` matches `tool_toml(name)`.
pub fn manifest_with(tool_names: &[&str]) -> String {
    let entries: Vec<serde_json::Value> = tool_names
        .iter()
        .map(|name| {
            let body = tool_toml(name);
            serde_json::json!({
                "name": name,
                "path": format!("tools/{name}.toml"),
                "sha256": sha256_hex(&body),
            })
        })
        .collect();
    serde_json::json!({
        "schema_version": 1,
        "tools": entries,
    })
    .to_string()
}

/// Routes a happy-path registry needs to serve: /manifest.json plus
/// /tools/{name}.toml for each name.
pub fn happy_routes(tool_names: &[&str]) -> HashMap<String, Canned> {
    let mut routes = HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        Canned::ok(manifest_with(tool_names)),
    );
    for name in tool_names {
        routes.insert(format!("/tools/{name}.toml"), Canned::ok(tool_toml(name)));
    }
    routes
}

// ===== Fake cosign shim =====

/// Generates a tiny shell script that pretends to be `cosign`. The
/// script reads env vars to decide what to do:
/// - `FAKE_COSIGN_VERIFY_EXIT` (default "0"): exit code for
///   `cosign verify-blob ...` invocations. Non-zero simulates rejection.
/// - `FAKE_COSIGN_STDERR` (default ""): text written to stderr on the
///   verify path. The registry sync surfaces this in the
///   `SyncError::Signature(reason)` variant.
/// - `FAKE_COSIGN_VERSION_EXIT` (default "0"): exit code for
///   `cosign version`. Set to non-zero to simulate cosign being broken
///   without removing it from PATH.
///
/// Returns the directory containing the shim binary so callers can
/// prepend it to PATH via `TestEnv::prepend_path`.
pub struct FakeCosign {
    pub dir: TempDir,
}

impl FakeCosign {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("fake cosign tempdir");
        let script_path = dir.path().join(if cfg!(windows) {
            "cosign.cmd"
        } else {
            "cosign"
        });

        #[cfg(unix)]
        {
            let script = r#"#!/bin/sh
# Fake cosign for jarvy test suite.
case "$1" in
  version)
    exit "${FAKE_COSIGN_VERSION_EXIT:-0}"
    ;;
  verify-blob)
    if [ -n "$FAKE_COSIGN_STDERR" ]; then
      printf '%s\n' "$FAKE_COSIGN_STDERR" >&2
    fi
    exit "${FAKE_COSIGN_VERIFY_EXIT:-0}"
    ;;
  *)
    echo "fake cosign: unknown subcommand $1" >&2
    exit 2
    ;;
esac
"#;
            std::fs::write(&script_path, script).expect("write fake cosign script");
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod fake cosign");
        }
        #[cfg(windows)]
        {
            let script = r#"@echo off
if "%1"=="version" exit /b %FAKE_COSIGN_VERSION_EXIT%
if "%1"=="verify-blob" (
  if defined FAKE_COSIGN_STDERR echo %FAKE_COSIGN_STDERR% 1>&2
  exit /b %FAKE_COSIGN_VERIFY_EXIT%
)
echo fake cosign: unknown subcommand %1 1>&2
exit /b 2
"#;
            std::fs::write(&script_path, script).expect("write fake cosign cmd");
        }

        FakeCosign { dir }
    }

    pub fn dir(&self) -> &Path {
        self.dir.path()
    }
}

// ===== Common dotted constants =====

pub const EXIT_CONFIG_ERROR: i32 = 2;
pub const EXIT_NETWORK_TIMEOUT: i32 = 4;

// ===== Shared `jarvy` Command builder =====

/// Build a `Command` for the test-built `jarvy` binary pre-wired with
/// the four env vars every registry test needs: `JARVY_HOME` pointing
/// at the test's sandbox, the loopback-HTTPS bypass enabled,
/// telemetry off, and test-mode on. Inherits PATH so the FakeCosign
/// shim (when prepended via `TestEnv::prepend_path`) is visible to
/// the child. PATH-inherit is harmless when no shim is in use.
pub fn jarvy_cmd(env: &TestEnv) -> std::process::Command {
    let mut c = std::process::Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_HOME", env.home())
        .env("JARVY_REGISTRY_ALLOW_INSECURE_FETCH", "1")
        .env("JARVY_TELEMETRY", "0")
        .env("JARVY_TEST_MODE", "1");
    if let Some(path) = std::env::var_os("PATH") {
        c.env("PATH", path);
    }
    c
}
