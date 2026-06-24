//! In-process integration tests for the remote tool-registry pull.
//!
//! These tests call `run_sync_with_config` directly (no `jarvy` child
//! process) against the in-process `MockRegistry` from
//! `tests/common/registry.rs`. The CLI-surface counterparts that
//! assert exit codes and stderr text live in:
//!   - `tests/registry_e2e_lifecycle.rs`
//!   - `tests/registry_signature_e2e.rs`
//!   - `tests/registry_resilience.rs`
//!   - `tests/registry_cli_smoke.rs`
//!
//! Split policy after the maintainability review (item P1 #5): each
//! invariant is tested at the layer that proves the most:
//!   - **SyncError variant** assertions live here (typed return value
//!     is the load-bearing surface for callers like the MCP layer).
//!   - **stderr text + CLI exit code** assertions live in the CLI
//!     suites (user-facing diagnostics are the load-bearing surface
//!     for operators).
//!
//! Tests that previously duplicated CLI-level coverage (happy zero
//! tools, parallel-fetch happy path) were removed — see
//! `tests/registry_resilience.rs` for the canonical versions.

#![allow(unsafe_code)] // env mutation fenced by #[serial]

mod common;

use std::collections::HashMap;

use serial_test::serial;

use common::registry::{Canned, MockRegistry, TestEnv, sha256_hex, tool_toml};

fn cfg(url: &str) -> jarvy::registry_remote::RegistryConfig {
    jarvy::registry_remote::RegistryConfig {
        url: url.into(),
        enabled: true,
        require_signature: false, // tests don't exercise cosign
        ..Default::default()
    }
}

// ===== Happy path: sync report + meta.json shape =====

#[test]
#[serial]
fn happy_one_tool_syncs_and_caches() {
    let _env = TestEnv::new();
    let tool_body = tool_toml("foo");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "foo", "path": "tools/foo.toml", "sha256": sha256_hex(&tool_body) }
        ]
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    routes.insert("/tools/foo.toml".to_string(), Canned::ok(tool_body.clone()));
    let server = MockRegistry::start(routes);

    let report = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url))
        .expect("sync should succeed");

    assert_eq!(report.tools_synced, 1);
    assert_eq!(report.tools_removed, 0);
    assert!(!report.signature_verified); // require_signature=false

    let tools_dir = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("tools");
    let foo_path = tools_dir.join("foo.toml");
    assert!(foo_path.exists(), "foo.toml should be cached");
    assert_eq!(std::fs::read(&foo_path).unwrap(), tool_body);

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

// (`happy_zero_tools_syncs` removed — covered at CLI level by
// `registry_resilience.rs::empty_manifest_is_valid_zero_tools_synced`,
// which asserts the same outcome plus the stdout report and exit code.)

// ===== Sha mismatch: pin the typed SyncError variant =====

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
    let server = MockRegistry::start(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url))
        .expect_err("sync should fail on sha mismatch");
    assert!(
        matches!(
            err,
            jarvy::registry_remote::SyncError::ShaMismatch { ref name, .. } if name == "foo"
        ),
        "got {err:?}"
    );
}

// ===== Fail-fast invariant: prior cache survives a failed sync =====

#[test]
#[serial]
fn partial_fetch_failure_preserves_prior_cache() {
    let _env = TestEnv::new();

    // Step 1: seed the cache with a successful prior sync.
    let foo_body = tool_toml("foo");
    let bar_body = tool_toml("bar");
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
    let server_1 = MockRegistry::start(routes_1);
    jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server_1.base_url))
        .expect("seed sync should succeed");
    drop(server_1);

    let tools_dir = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("tools");
    assert!(tools_dir.join("foo.toml").exists());
    assert!(tools_dir.join("bar.toml").exists());

    // Step 2: a sync where the second tool 404s.
    let baz_body = tool_toml("baz");
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
    let server_2 = MockRegistry::start(routes_2);
    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server_2.base_url))
        .expect_err("sync should fail on the qux 404");
    assert!(matches!(err, jarvy::registry_remote::SyncError::Fetch(_)));

    // Prior foo + bar still present; baz/qux NOT.
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

// ===== Trust narrowing: project config can't subscribe =====

#[test]
#[serial]
fn registry_config_is_global_only_not_project() {
    let _env = TestEnv::new();
    assert!(jarvy::registry_remote::RegistryConfig::load().is_none());

    let cwd_jarvy = std::env::current_dir().unwrap().join("jarvy.toml.test");
    let _ = std::fs::write(
        &cwd_jarvy,
        r#"
[registry]
url = "https://attacker.example/r/"
enabled = true
"#,
    );
    assert!(
        jarvy::registry_remote::RegistryConfig::load().is_none(),
        "project-level jarvy.toml MUST NOT subscribe to a registry"
    );
    let _ = std::fs::remove_file(&cwd_jarvy);
}

// ===== Schema-version too new: pin ManifestError::UnsupportedSchema =====

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
    let server = MockRegistry::start(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url))
        .expect_err("v99 manifest should be refused");
    assert!(matches!(
        err,
        jarvy::registry_remote::SyncError::Manifest(
            jarvy::registry_remote::ManifestError::UnsupportedSchema { found: 99, .. }
        )
    ));
}

// ===== HTTPS redirect refused at the shared agent =====

#[test]
#[serial]
fn https_redirect_is_refused() {
    let _env = TestEnv::new();
    let mut routes = HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        Canned::redirect("http://attacker.example/manifest.json"),
    );
    let server = MockRegistry::start(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url))
        .expect_err("redirect should NOT be auto-followed");
    // shared net::agent is configured max_redirects(0); ureq surfaces
    // this as a non-200 / TooManyRedirects error. We only require:
    // sync errored, didn't follow.
    assert!(
        matches!(err, jarvy::registry_remote::SyncError::Fetch(_)),
        "got {err:?}"
    );
}

// ===== Plugin-loader cache index =====

#[test]
#[serial]
fn sync_writes_remote_index_for_plugin_loader_cache() {
    let _env = TestEnv::new();
    let tool_body = tool_toml("indexed");
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
    let server = MockRegistry::start(routes);

    jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url))
        .expect("sync should succeed");

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

    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cache_root.join("meta.json")).unwrap()).unwrap();
    assert_eq!(index["synced_at_unix"], meta["last_synced_at_unix"]);
}

// (`parallel_fetch_handles_many_tools` removed — covered at CLI level
// by `registry_resilience.rs::many_tools_parallel_fetch_succeeds`
// with 32 tools instead of 25 and per-tool hit-count assertions.)

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
        let body = tool_toml(&name);
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
    let server = MockRegistry::start(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url))
        .expect_err("the 404 on tool_5 should propagate");
    assert!(matches!(err, jarvy::registry_remote::SyncError::Fetch(_)));

    let tools_dir = jarvy::paths::registry_remote_cache_dir()
        .unwrap()
        .join("tools");
    assert!(
        !tools_dir.exists() || std::fs::read_dir(&tools_dir).unwrap().count() == 0,
        "fail-fast: no tools/ files should be visible after a partial sync"
    );
}

// ===== Duplicate-name manifest rejected at parse: pin variant =====

#[test]
#[serial]
fn duplicate_tool_names_in_manifest_are_rejected() {
    let _env = TestEnv::new();
    let body = tool_toml("dup");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "dup", "path": "tools/a.toml", "sha256": sha256_hex(&body) },
            { "name": "dup", "path": "tools/b.toml", "sha256": sha256_hex(&body) },
        ]
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    let server = MockRegistry::start(routes);

    let err = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url))
        .expect_err("duplicate names must be refused");
    assert!(
        matches!(
            err,
            jarvy::registry_remote::SyncError::Manifest(
                jarvy::registry_remote::ManifestError::DuplicateName { ref name }
            ) if name == "dup"
        ),
        "expected ManifestError::DuplicateName('dup'); got {err:?}"
    );
}

// ===== Index build failure is non-fatal =====

#[test]
#[serial]
#[cfg(unix)]
fn sync_still_succeeds_when_index_build_fails() {
    use std::os::unix::fs::PermissionsExt;
    let _env = TestEnv::new();
    let tool_body = tool_toml("foo");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [
            { "name": "foo", "path": "tools/foo.toml", "sha256": sha256_hex(&tool_body) }
        ]
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    routes.insert("/tools/foo.toml".to_string(), Canned::ok(tool_body));
    let server = MockRegistry::start(routes);

    let cache_dir = jarvy::paths::registry_remote_cache_dir().unwrap();
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::set_permissions(&cache_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let index_path = cache_dir.join("index.json");
    std::fs::write(&index_path, b"placeholder").unwrap();
    std::fs::set_permissions(&cache_dir, std::fs::Permissions::from_mode(0o500)).unwrap();

    let report = jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url));
    // Restore so test teardown can clean up.
    std::fs::set_permissions(&cache_dir, std::fs::Permissions::from_mode(0o700)).unwrap();

    let report = report.expect("sync must report Ok even when index build fails");
    assert_eq!(report.tools_synced, 1);
}

// ===== meta.json schema =====

#[test]
#[serial]
fn meta_json_records_required_fields() {
    let _env = TestEnv::new();
    let tool_body = tool_toml("alpha");
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
    let server = MockRegistry::start(routes);

    jarvy::registry_remote::sync::run_sync_with_config(&cfg(&server.base_url))
        .expect("sync should succeed");

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
