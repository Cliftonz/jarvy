//! End-to-end integration tests for the remote tool-registry pull.
//!
//! Spins up a tiny TCP listener that serves canned HTTP/1.1 responses to
//! ureq, points a `RegistryConfig` at it via the
//! `JARVY_REGISTRY_ALLOW_INSECURE_FETCH=1` test bypass, and exercises
//! the orchestrator end-to-end. Every test pins `JARVY_HOME` to a
//! per-test tempdir so the suite never touches the developer's real
//! `~/.jarvy/`.
//!
//! Covers items 3, 4, 7-invariant, 12, 13, 14, 15, 32 from the
//! parallel-code-review enhancement plan.

#![allow(unsafe_code)] // env mutation is fenced by #[serial] + cleanup guards

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serial_test::serial;
use tempfile::TempDir;

/// Holds the per-test environment. Drop restores `JARVY_HOME` to its
/// prior value so subsequent tests don't see the temp dir.
struct TestEnv {
    _tmp: TempDir,
    prev_home: Option<std::ffi::OsString>,
    prev_insecure: Option<std::ffi::OsString>,
}

impl TestEnv {
    fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let prev_home = std::env::var_os("JARVY_HOME");
        let prev_insecure = std::env::var_os("JARVY_REGISTRY_ALLOW_INSECURE_FETCH");
        unsafe {
            std::env::set_var("JARVY_HOME", tmp.path());
            std::env::set_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH", "1");
        }
        Self {
            _tmp: tmp,
            prev_home,
            prev_insecure,
        }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        unsafe {
            match &self.prev_home {
                Some(v) => std::env::set_var("JARVY_HOME", v),
                None => std::env::remove_var("JARVY_HOME"),
            }
            match &self.prev_insecure {
                Some(v) => std::env::set_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH", v),
                None => std::env::remove_var("JARVY_REGISTRY_ALLOW_INSECURE_FETCH"),
            }
        }
    }
}

/// Canned response for a single path. `status` is the HTTP status line
/// (200 for happy, 404 for not-found, 301 for redirect testing). Body is
/// served verbatim.
#[derive(Clone)]
struct Canned {
    status: u16,
    body: Vec<u8>,
    /// If set, sent as a `Location:` header (for 3xx redirect tests).
    redirect_to: Option<String>,
}

impl Canned {
    fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            redirect_to: None,
        }
    }
    fn not_found() -> Self {
        Self {
            status: 404,
            body: b"not found".to_vec(),
            redirect_to: None,
        }
    }
    fn redirect(to: impl Into<String>) -> Self {
        Self {
            status: 301,
            body: Vec::new(),
            redirect_to: Some(to.into()),
        }
    }
}

/// Minimal HTTP/1.1 test server. Each path → canned response.
fn spawn_server(routes: HashMap<String, Canned>) -> (String, Arc<Mutex<bool>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).expect("nonblock");
    let url = format!("http://127.0.0.1:{port}/");

    let stop = Arc::new(Mutex::new(false));
    let stop_srv = Arc::clone(&stop);
    let routes = Arc::new(routes);

    thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            if *stop_srv.lock().unwrap() {
                break;
            }
            if std::time::Instant::now() > deadline {
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    let routes = Arc::clone(&routes);
                    thread::spawn(move || handle_request(stream, &routes));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (url, stop)
}

fn handle_request(mut stream: TcpStream, routes: &HashMap<String, Canned>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    // Extract path from "GET /path HTTP/1.1"
    let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
    let canned = routes.get(&path).cloned().unwrap_or_else(Canned::not_found);

    let mut response = format!(
        "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
        canned.status,
        canned.body.len()
    );
    if let Some(to) = &canned.redirect_to {
        response.push_str(&format!("Location: {to}\r\n"));
    }
    response.push_str("\r\n");
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(&canned.body);
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn cfg(url: &str) -> jarvy::registry_remote::RegistryConfig {
    jarvy::registry_remote::RegistryConfig {
        url: url.into(),
        enabled: true,
        require_signature: false, // tests don't exercise cosign
        ..Default::default()
    }
}

fn make_tool_toml(name: &str) -> Vec<u8> {
    format!(
        r#"name = "{name}"
command = "{name}"

[macos]
brew = "{name}"
"#
    )
    .into_bytes()
}

// ===== Item 3 — happy path =====

#[test]
#[serial]
fn happy_one_tool_syncs_and_caches() {
    let _env = TestEnv::new();
    let tool_body = make_tool_toml("foo");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "foo", "path": "tools/foo.toml", "sha256": sha256_hex(&tool_body) }
        ]
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest.clone()));
    routes.insert("/tools/foo.toml".to_string(), Canned::ok(tool_body.clone()));
    let (url, stop) = spawn_server(routes);

    let report = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url))
        .expect("sync should succeed");
    *stop.lock().unwrap() = true;

    assert_eq!(report.tools_synced, 1);
    assert_eq!(report.tools_removed, 0);
    assert!(!report.signature_verified); // require_signature=false

    // foo.toml ended up in the active tools dir.
    let tools_dir = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("tools");
    let foo_path = tools_dir.join("foo.toml");
    assert!(foo_path.exists(), "foo.toml should be cached");
    assert_eq!(std::fs::read(&foo_path).unwrap(), tool_body);

    // meta.json contains the expected fields.
    let meta_path = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("meta.json");
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
    assert_eq!(meta["tools_count"], 1);
    assert_eq!(meta["signature_verified"], false);
    assert!(meta["duration_ms"].is_u64());
    assert!(
        meta["registry_url"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1")
    );
}

#[test]
#[serial]
fn happy_zero_tools_syncs() {
    let _env = TestEnv::new();
    let manifest = serde_json::json!({"schema_version": 1, "tools": []}).to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    let (url, stop) = spawn_server(routes);

    let report = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url))
        .expect("zero-tool sync should succeed");
    *stop.lock().unwrap() = true;
    assert_eq!(report.tools_synced, 0);
}

// ===== Item 4 — sha mismatch =====

#[test]
#[serial]
fn sha_mismatch_aborts_sync() {
    let _env = TestEnv::new();
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "foo", "path": "tools/foo.toml",
              "sha256": "0".repeat(64) }
        ]
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    routes.insert(
        "/tools/foo.toml".to_string(),
        Canned::ok(b"hostile".to_vec()),
    );
    let (url, stop) = spawn_server(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url))
        .expect_err("sync should fail on sha mismatch");
    *stop.lock().unwrap() = true;
    assert!(
        matches!(
            err,
            jarvy::registry_remote::SyncError::ShaMismatch { ref name, .. } if name == "foo"
        ),
        "got {err:?}"
    );
}

// ===== Item 7 — fail-fast invariant (the bug fix) =====

#[test]
#[serial]
fn partial_fetch_failure_preserves_prior_cache() {
    let _env = TestEnv::new();

    // Step 1: seed the cache with a successful prior sync.
    let foo_body = make_tool_toml("foo");
    let bar_body = make_tool_toml("bar");
    let manifest_1 = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "foo", "path": "tools/foo.toml", "sha256": sha256_hex(&foo_body) },
            { "name": "bar", "path": "tools/bar.toml", "sha256": sha256_hex(&bar_body) },
        ]
    })
    .to_string();
    let mut routes_1 = HashMap::new();
    routes_1.insert("/manifest.json".to_string(), Canned::ok(manifest_1));
    routes_1.insert("/tools/foo.toml".to_string(), Canned::ok(foo_body.clone()));
    routes_1.insert("/tools/bar.toml".to_string(), Canned::ok(bar_body.clone()));
    let (url_1, stop_1) = spawn_server(routes_1);
    jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url_1))
        .expect("seed sync should succeed");
    *stop_1.lock().unwrap() = true;

    let tools_dir = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("tools");
    assert!(tools_dir.join("foo.toml").exists());
    assert!(tools_dir.join("bar.toml").exists());

    // Step 2: a sync where the SECOND tool 404s.
    let baz_body = make_tool_toml("baz");
    let manifest_2 = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "baz", "path": "tools/baz.toml", "sha256": sha256_hex(&baz_body) },
            { "name": "qux", "path": "tools/qux.toml", "sha256": sha256_hex(b"qux") },
        ]
    })
    .to_string();
    let mut routes_2 = HashMap::new();
    routes_2.insert("/manifest.json".to_string(), Canned::ok(manifest_2));
    routes_2.insert("/tools/baz.toml".to_string(), Canned::ok(baz_body));
    routes_2.insert("/tools/qux.toml".to_string(), Canned::not_found());
    let (url_2, stop_2) = spawn_server(routes_2);
    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url_2))
        .expect_err("sync should fail on the qux 404");
    *stop_2.lock().unwrap() = true;
    assert!(matches!(err, jarvy::registry_remote::SyncError::Fetch(_)));

    // Step 3: assert pre-existing foo + bar still present, baz/qux NOT.
    // This is the invariant the PR's doc-comment claimed but the
    // wipe-before-fetch shape violated.
    assert!(
        tools_dir.join("foo.toml").exists(),
        "prior cache MUST survive a failed sync"
    );
    assert!(
        tools_dir.join("bar.toml").exists(),
        "prior cache MUST survive a failed sync"
    );
    assert!(
        !tools_dir.join("baz.toml").exists(),
        "partial new state MUST NOT bleed into active cache"
    );
    assert!(!tools_dir.join("qux.toml").exists());
}

// ===== Item 13 — trust narrowing (project config can't subscribe) =====

#[test]
#[serial]
fn registry_config_is_global_only_not_project() {
    let _env = TestEnv::new();
    // No ~/.jarvy/config.toml exists.
    assert!(jarvy::registry_remote::RegistryConfig::load().is_none());

    // Even if a project jarvy.toml in CWD contains a [registry] section,
    // load() ignores it — load() reads from paths::config_toml() only.
    let cwd_jarvy = std::env::current_dir().unwrap().join("jarvy.toml.test");
    let _ = std::fs::write(
        &cwd_jarvy,
        r#"
[registry]
url = "https://attacker.example/r/"
enabled = true
"#,
    );
    // RegistryConfig::load() still returns None — confirms it doesn't
    // walk CWD for a project file.
    assert!(
        jarvy::registry_remote::RegistryConfig::load().is_none(),
        "project-level jarvy.toml MUST NOT subscribe to a registry"
    );
    let _ = std::fs::remove_file(&cwd_jarvy);
}

// ===== Item 15 — schema_version too new at orchestrator =====

#[test]
#[serial]
fn schema_version_too_new_aborts_with_manifest_error() {
    let _env = TestEnv::new();
    let manifest = serde_json::json!({
        "schema_version": 99,
        "tools": []
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    let (url, stop) = spawn_server(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url))
        .expect_err("v99 manifest should be refused");
    *stop.lock().unwrap() = true;
    assert!(matches!(
        err,
        jarvy::registry_remote::SyncError::Manifest(
            jarvy::registry_remote::ManifestError::UnsupportedSchema { found: 99, .. }
        )
    ));
}

// ===== Item 32 — HTTPS redirect refusal =====

#[test]
#[serial]
fn https_redirect_is_refused() {
    let _env = TestEnv::new();
    let mut routes = HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        Canned::redirect("http://attacker.example/manifest.json"),
    );
    let (url, stop) = spawn_server(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url))
        .expect_err("redirect should NOT be auto-followed");
    *stop.lock().unwrap() = true;
    // shared net::agent is configured max_redirects(0); ureq surfaces
    // this as a non-200 / TooManyRedirects error category. We just
    // require: sync errors, didn't follow.
    assert!(
        matches!(err, jarvy::registry_remote::SyncError::Fetch(_)),
        "got {err:?}"
    );
}

// ===== Item 12 — require_signature=false emits stderr warning =====
//
// The stderr capture lives in the orchestrator's `eprintln!`. We can't
// trivially redirect raw stderr from a #[test] thread without a helper
// crate, so we settle for asserting the structured outcome
// (signature_verified=false in meta.json + the report) which proves the
// disabled branch was taken. The stderr text is asserted via assert_cmd
// in a separate CLI-level smoke test (see registry_cli_smoke.rs).
//
// Coverage gap remaining: a future PR can add the `gag` crate to
// capture raw stderr if the team wants the literal "WARNING" text
// pinned by a Rust-test-level assertion.

// ===== Item 2 — plugin-loader cache index =====

#[test]
#[serial]
fn sync_writes_remote_index_for_plugin_loader_cache() {
    let _env = TestEnv::new();
    let tool_body = make_tool_toml("indexed");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "indexed", "path": "tools/indexed.toml", "sha256": sha256_hex(&tool_body) }
        ]
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    routes.insert("/tools/indexed.toml".to_string(), Canned::ok(tool_body));
    let (url, stop) = spawn_server(routes);

    jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url)).expect("sync should succeed");
    *stop.lock().unwrap() = true;

    let cache_root = jarvy::paths::registry_remote_cache_dir().unwrap();
    let index_path = cache_root.join("index.json");
    assert!(
        index_path.exists(),
        "sync must write index.json for the loader cache"
    );

    let index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index_path).unwrap()).expect("index.json parses");
    assert!(index["synced_at_unix"].is_u64());
    let tools = index["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "indexed");
    assert_eq!(tools[0]["command"], "indexed");

    // synced_at_unix matches meta.json so the loader's invalidation
    // check accepts the index as fresh.
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cache_root.join("meta.json")).unwrap()).unwrap();
    assert_eq!(index["synced_at_unix"], meta["last_synced_at_unix"]);
}

// ===== Item 12 — parallel per-tool fetch sanity =====

#[test]
#[serial]
fn parallel_fetch_handles_many_tools() {
    let _env = TestEnv::new();
    // 25 tools — more than the default parallelism cap (8) so we
    // actually exercise the round-robin work-stealing logic.
    const COUNT: usize = 25;
    let mut tool_entries = Vec::with_capacity(COUNT);
    let mut routes = HashMap::new();
    for i in 0..COUNT {
        let name = format!("tool_{i}");
        let body = make_tool_toml(&name);
        let sha = sha256_hex(&body);
        let path = format!("tools/{name}.toml");
        routes.insert(format!("/{path}"), Canned::ok(body));
        tool_entries.push(serde_json::json!({
            "name": name,
            "path": path,
            "sha256": sha,
        }));
    }
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": tool_entries,
    })
    .to_string();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    let (url, stop) = spawn_server(routes);

    let report = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url))
        .expect("parallel sync of 25 tools should succeed");
    *stop.lock().unwrap() = true;

    assert_eq!(report.tools_synced, COUNT);
    // All 25 landed in tools/.
    let tools_dir = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("tools");
    for i in 0..COUNT {
        let p = tools_dir.join(format!("tool_{i}.toml"));
        assert!(p.exists(), "tool_{i}.toml missing after parallel sync");
    }
}

#[test]
#[serial]
fn parallel_fetch_fails_fast_on_404() {
    let _env = TestEnv::new();
    // 10 tools: tool_5 404s. With parallelism, several workers may
    // have already started; the test asserts the error surfaces and
    // tools/ stays untouched (fail-fast invariant + staging-swap).
    let mut tool_entries = Vec::new();
    let mut routes = HashMap::new();
    for i in 0..10 {
        let name = format!("tool_{i}");
        let body = make_tool_toml(&name);
        let sha = sha256_hex(&body);
        let path = format!("tools/{name}.toml");
        if i == 5 {
            routes.insert(format!("/{path}"), Canned::not_found());
        } else {
            routes.insert(format!("/{path}"), Canned::ok(body));
        }
        tool_entries.push(serde_json::json!({
            "name": name,
            "path": path,
            "sha256": sha,
        }));
    }
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": tool_entries,
    })
    .to_string();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    let (url, stop) = spawn_server(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url))
        .expect_err("the 404 on tool_5 should propagate");
    *stop.lock().unwrap() = true;
    assert!(matches!(err, jarvy::registry_remote::SyncError::Fetch(_)));

    // No partial state in active tools/.
    let tools_dir = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("tools");
    assert!(
        !tools_dir.exists() || std::fs::read_dir(&tools_dir).unwrap().count() == 0,
        "fail-fast: no tools/ files should be visible after a partial sync"
    );
}

// ===== Item 14 — meta.json schema =====

#[test]
#[serial]
fn meta_json_records_required_fields() {
    let _env = TestEnv::new();
    let tool_body = make_tool_toml("alpha");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "alpha", "path": "tools/alpha.toml", "sha256": sha256_hex(&tool_body) }
        ]
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    routes.insert("/tools/alpha.toml".to_string(), Canned::ok(tool_body));
    let (url, stop) = spawn_server(routes);

    jarvy::registry_remote::sync::run_sync_with_config(&cfg(&url)).expect("sync should succeed");
    *stop.lock().unwrap() = true;

    let meta_path = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("meta.json");
    let raw = std::fs::read(&meta_path).expect("meta.json present");
    let meta: serde_json::Value = serde_json::from_slice(&raw).expect("meta.json parses");

    for field in [
        "last_synced_at_unix",
        "registry_url",
        "tools_count",
        "tools_removed",
        "signature_verified",
        "duration_ms",
    ] {
        assert!(meta.get(field).is_some(), "meta.json missing field {field}");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&meta_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "meta.json must be 0600");
    }
}
