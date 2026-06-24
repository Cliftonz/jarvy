//! End-to-end lifecycle tests for the remote tool-registry pull.
//!
//! Exercises the full user journey via the real `jarvy` binary against
//! the in-process `MockRegistry`:
//!
//! 1. User writes `[registry]` config
//! 2. `jarvy registry sync` pulls + caches
//! 3. `jarvy registry status` shows the sync
//! 4. The plugin loader sees the synced tools on next startup
//! 5. `jarvy registry clear` wipes the cache
//!
//! These are the paths every registry user will hit. They run the
//! actual binary (`assert_cmd::cargo_bin!`) so any wiring break between
//! `clap` parsing, the orchestrator, and the cache layer surfaces here.

#![allow(unsafe_code)] // env mutation fenced by #[serial(registry_env)]

mod common;

use serial_test::serial;

use common::registry::{
    EXIT_CONFIG_ERROR, MockRegistry, TestEnv, happy_routes, jarvy_cmd as jarvy, tool_toml,
};

// ===== Happy path: configure → sync → status → clear =====

#[test]
#[serial(registry_env)]
fn full_lifecycle_sync_status_clear() {
    let env = TestEnv::new();
    let server = MockRegistry::start(happy_routes(&["alpha", "beta"]));
    env.write_registry_config(&server.base_url, false);

    // 1. Sync — exit 0, stdout reports counts.
    let sync = jarvy(&env)
        .arg("registry")
        .arg("sync")
        .output()
        .expect("spawn jarvy registry sync");
    assert!(
        sync.status.success(),
        "sync must exit 0; got {:?}\nstderr: {}",
        sync.status.code(),
        String::from_utf8_lossy(&sync.stderr),
    );
    let sync_stdout = String::from_utf8_lossy(&sync.stdout);
    assert!(
        sync_stdout.contains("Tools synced:     2"),
        "stdout should report synced count; got:\n{sync_stdout}"
    );
    assert!(
        sync_stdout.contains("[ok] Registry sync complete"),
        "stdout should announce completion; got:\n{sync_stdout}"
    );

    // Cache state: both tools landed in tools/, index.json + meta.json present.
    let tools_dir = env.tools_dir();
    assert!(
        tools_dir.join("alpha.toml").exists(),
        "alpha.toml must exist"
    );
    assert!(tools_dir.join("beta.toml").exists(), "beta.toml must exist");
    assert!(
        env.cache_dir().join("meta.json").exists(),
        "meta.json must exist"
    );
    assert!(
        env.cache_dir().join("index.json").exists(),
        "index.json must exist for the loader cache"
    );

    // 2. Status — exit 0, shows meta.json contents.
    let status = jarvy(&env)
        .arg("registry")
        .arg("status")
        .output()
        .expect("spawn jarvy registry status");
    assert!(status.status.success());
    let status_stdout = String::from_utf8_lossy(&status.stdout);
    assert!(
        status_stdout.contains("\"tools_count\":2"),
        "status should show 2 tools; got:\n{status_stdout}"
    );
    assert!(
        status_stdout.contains("\"signature_verified\":false"),
        "status should reflect unsigned sync; got:\n{status_stdout}"
    );

    // Hit-count sanity: exactly one manifest + one fetch per tool.
    assert_eq!(server.hits_for("/manifest.json"), 1);
    assert_eq!(server.hits_for("/tools/alpha.toml"), 1);
    assert_eq!(server.hits_for("/tools/beta.toml"), 1);

    // 3. Clear — exit 0, cache dir gone.
    let clear = jarvy(&env)
        .arg("registry")
        .arg("clear")
        .output()
        .expect("spawn jarvy registry clear");
    assert!(clear.status.success());
    assert!(
        !env.cache_dir().exists(),
        "cache dir must be wiped by clear"
    );

    // 4. Status after clear — exit 0, hints to re-sync.
    let status2 = jarvy(&env)
        .arg("registry")
        .arg("status")
        .output()
        .expect("spawn jarvy registry status (post-clear)");
    assert!(status2.status.success());
    let status2_stdout = String::from_utf8_lossy(&status2.stdout);
    assert!(
        status2_stdout.contains("No registry sync recorded"),
        "post-clear status should hint to sync; got:\n{status2_stdout}"
    );
}

// ===== Idempotency: sync twice with no upstream changes =====

#[test]
#[serial(registry_env)]
fn sync_twice_with_no_changes_is_idempotent() {
    let env = TestEnv::new();
    let server = MockRegistry::start(happy_routes(&["foo"]));
    env.write_registry_config(&server.base_url, false);

    // First sync.
    let first = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(first.status.success(), "first sync must succeed");
    let first_stdout = String::from_utf8_lossy(&first.stdout).into_owned();
    assert!(first_stdout.contains("Tools synced:     1"));
    assert!(first_stdout.contains("Tools removed:    0"));

    // Second sync — same routes, same shas.
    let second = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        second.status.success(),
        "second sync must also succeed; got {:?}\nstderr: {}",
        second.status.code(),
        String::from_utf8_lossy(&second.stderr),
    );
    let second_stdout = String::from_utf8_lossy(&second.stdout).into_owned();
    assert!(second_stdout.contains("Tools synced:     1"));
    assert!(second_stdout.contains("Tools removed:    0"));

    // Cache contents identical.
    assert!(env.tools_dir().join("foo.toml").exists());

    // Server saw exactly two manifest fetches (one per sync) and two
    // tool fetches.
    assert_eq!(server.hits_for("/manifest.json"), 2);
    assert_eq!(server.hits_for("/tools/foo.toml"), 2);
}

// ===== Upstream removes a tool: removed count + cache reflects =====

#[test]
#[serial(registry_env)]
fn upstream_removes_tool_is_reflected_in_removed_count() {
    let env = TestEnv::new();

    // First sync: 2 tools.
    let server1 = MockRegistry::start(happy_routes(&["alpha", "beta"]));
    env.write_registry_config(&server1.base_url, false);
    let first = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(first.status.success());
    drop(server1);

    // Second sync: only alpha. beta should disappear, removed_count = 1.
    let server2 = MockRegistry::start(happy_routes(&["alpha"]));
    // Need to rewrite config to point at the new server port.
    env.write_registry_config(&server2.base_url, false);
    let second = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(second.status.success(), "second sync must succeed");
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("Tools synced:     1"),
        "synced count should be 1; got:\n{stdout}"
    );
    assert!(
        stdout.contains("Tools removed:    1"),
        "removed count should be 1 (beta dropped); got:\n{stdout}"
    );

    // Cache reflects the drop.
    assert!(env.tools_dir().join("alpha.toml").exists());
    assert!(!env.tools_dir().join("beta.toml").exists());
}

// ===== Loader cache layout: synced data on disk matches what the plugin
// loader reads on next startup =====
//
// `jarvy tools --index` only enumerates the inventory built by the
// `define_tool!` macro + MANUAL_TOOLS, not plugin-registered tools, so
// it can't witness a synced tool. Instead this test pins the cache
// layout that `plugins::try_load_remote_index` + `load_tools_from_dir`
// consume: the per-tool TOML and the JSON index. If either drifts, the
// loader silently stops seeing remote tools on the next startup.

#[test]
#[serial(registry_env)]
fn synced_tool_cache_matches_loader_contract() {
    let env = TestEnv::new();
    let server = MockRegistry::start(happy_routes(&["loader-test-tool"]));
    env.write_registry_config(&server.base_url, false);

    let sync = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        sync.status.success(),
        "sync must succeed; stderr:\n{}",
        String::from_utf8_lossy(&sync.stderr)
    );

    // Per-tool TOML — what load_tools_from_dir() walks.
    let tool_path = env.tools_dir().join("loader-test-tool.toml");
    let tool_body = std::fs::read_to_string(&tool_path)
        .unwrap_or_else(|e| panic!("expected {} on disk: {e}", tool_path.display()));
    assert!(
        tool_body.contains(r#"name = "loader-test-tool""#),
        "tool TOML missing canonical name field; got:\n{tool_body}"
    );

    // Parsed JSON index — what try_load_remote_index() prefers over the
    // walk. Must contain the tool name so the loader registers it.
    let index_path = env.cache_dir().join("index.json");
    let index_raw = std::fs::read_to_string(&index_path)
        .unwrap_or_else(|e| panic!("expected {} on disk: {e}", index_path.display()));
    let index: serde_json::Value =
        serde_json::from_str(&index_raw).expect("index.json must be valid JSON");
    let tools = index["tools"]
        .as_array()
        .expect("index.json: tools field must be an array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        names.contains(&"loader-test-tool"),
        "synced tool MUST appear in index.json `tools[]`; got names: {names:?}"
    );
}

// ===== Misconfig surfaces clear errors =====

#[test]
#[serial(registry_env)]
fn sync_without_config_exits_clean() {
    let env = TestEnv::new();
    // No config.toml written.
    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert_eq!(out.status.code(), Some(EXIT_CONFIG_ERROR));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not configured"),
        "stderr should explain missing config; got:\n{stderr}"
    );
}

#[test]
#[serial(registry_env)]
fn sync_with_http_config_refused_at_validate_safety() {
    let env = TestEnv::new();
    // Even though JARVY_REGISTRY_ALLOW_INSECURE_FETCH=1 is set, the
    // CONFIG (not a per-fetch URL) is non-loopback http://; validate_safety
    // refuses. The bypass is per-fetch, not config-level.
    env.write_registry_config("http://example.com/r/", false);
    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(EXIT_CONFIG_ERROR),
        "non-https + non-loopback config must be refused; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("https") || stderr.contains("HTTPS"),
        "stderr should mention https requirement; got:\n{stderr}"
    );
}

// `sync_with_unanchored_regex_refused` deliberately lives only in
// `tests/registry_cli_smoke.rs:187` — same invariant, stricter
// stderr assertion (`"fully anchored"`). Two tests with the same name
// across binaries were a maintainability finding from the parallel
// code review (item P2 #12).

// ===== Redirect refusal: the shared agent must not follow 3xx =====
//
// `crate::net::agent::agent()` is configured with `max_redirects(0)`.
// A regression that bumped it would silently let a hostile registry
// redirect /manifest.json to an attacker-controlled URL — and the
// signed-path chain (sig + pem + tool fetches) would all follow too.
// Pin the contract from the CLI surface.

#[test]
#[serial(registry_env)]
fn sync_refuses_to_follow_manifest_redirect() {
    let env = TestEnv::new();
    let mut routes = std::collections::HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        common::registry::Canned::redirect("http://attacker.example/evil-manifest.json"),
    );
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "redirect must NOT be followed; sync must fail. got exit {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    // The hostile URL must NOT have been resolved — only the original
    // manifest URL was hit (exactly once, the 301).
    assert_eq!(
        server.hits_for("/manifest.json"),
        1,
        "manifest URL should be hit exactly once (the 301 response)"
    );
    // Cache must be empty — no manifest promoted from the redirect target.
    assert!(!env.cache_dir().join("manifest.json").exists());
}

// ===== Delayed response: sync still succeeds when the server is slow
// but stays inside the agent's read-body timeout. Pins the
// `Canned::delayed` knob and proves the harness can model slow
// upstreams. (Triggering the actual agent timeout requires a 30 s
// delay, which would be a sluggish CI test — out of scope here.) =====

#[test]
#[serial(registry_env)]
fn sync_succeeds_against_briefly_delayed_server() {
    let env = TestEnv::new();
    let manifest_body = common::registry::manifest_with(&["slow-tool"]);
    let tool_body = tool_toml("slow-tool");

    let mut routes = std::collections::HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        common::registry::Canned::ok(manifest_body).delayed(std::time::Duration::from_millis(250)),
    );
    routes.insert(
        "/tools/slow-tool.toml".to_string(),
        common::registry::Canned::ok(tool_body),
    );
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let started = std::time::Instant::now();
    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    let elapsed = started.elapsed();

    assert!(
        out.status.success(),
        "delayed-but-not-timed-out sync must succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Wallclock check: the 250 ms server-side delay must have actually
    // landed. A regression that bypassed `Canned::delayed` would
    // complete in <50 ms.
    assert!(
        elapsed >= std::time::Duration::from_millis(200),
        "delayed response must take at least ~250 ms; observed {:?}",
        elapsed
    );
    assert!(env.tools_dir().join("slow-tool.toml").exists());
}

// ===== Sha mismatch surfaces clearly =====

#[test]
#[serial(registry_env)]
fn sha_mismatch_aborts_with_clear_stderr() {
    let env = TestEnv::new();
    // Manifest claims a sha; server serves DIFFERENT content for the
    // tool URL.
    let real_body = tool_toml("mismatched");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [{
            "name": "mismatched",
            "path": "tools/mismatched.toml",
            "sha256": common::registry::sha256_hex(&real_body),
        }],
    })
    .to_string();
    let mut routes = std::collections::HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        common::registry::Canned::ok(manifest),
    );
    // Serve TAMPERED body (sha won't match).
    routes.insert(
        "/tools/mismatched.toml".to_string(),
        common::registry::Canned::ok(b"hostile bytes".to_vec()),
    );
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "sync must fail on sha mismatch; got exit {:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("sha256 mismatch") && stderr.contains("mismatched"),
        "stderr should name the bad tool; got:\n{stderr}"
    );

    // Active cache MUST be empty — staging-swap fail-fast invariant.
    assert!(
        !env.tools_dir().exists() || std::fs::read_dir(env.tools_dir()).unwrap().count() == 0,
        "no files should be visible in active tools/ after fail-fast"
    );
}
