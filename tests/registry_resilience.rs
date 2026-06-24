//! Resilience tests for `jarvy registry sync` — exercises the failure
//! modes that the orchestrator MUST handle without leaving the cache in
//! a half-written state.
//!
//! Covered:
//! - Oversized manifest response (MAX_MANIFEST_BYTES cap)
//! - Oversized per-tool TOML response (MAX_TOOL_BYTES cap)
//! - Truncated response body (connection drop mid-stream)
//! - HTTP 500 on manifest
//! - HTTP 500 on one of many tools (partial-failure aborts whole sync)
//! - Many-tool parallel fetch stress (32 tools)
//! - Recovery after failure — first sync fails, second sync cleans up
//!   prior `.unverified` and succeeds
//! - Manifest with malformed JSON
//! - Manifest with duplicate tool names (race-safe rejection)
//!
//! Each test uses the real `jarvy` binary so the actual fail-fast and
//! atomic-swap invariants are observed in production code paths, not
//! re-implemented in test logic.

#![allow(unsafe_code)] // env mutation fenced by #[serial(registry_env)]

mod common;

use std::collections::HashMap;

use serial_test::serial;

use common::registry::{
    Canned, EXIT_CONFIG_ERROR, EXIT_NETWORK_TIMEOUT, MockRegistry, TestEnv, happy_routes,
    jarvy_cmd as jarvy, manifest_with, sha256_hex, tool_toml,
};

// ===== Oversized manifest: MAX_MANIFEST_BYTES = 5 MiB =====

#[test]
#[serial(registry_env)]
fn oversized_manifest_is_rejected_by_cap() {
    let env = TestEnv::new();
    // Build a manifest body well above MAX_MANIFEST_BYTES (5 MiB). The
    // body must still START with valid JSON so a non-cap-aware client
    // would parse it; the cap is what protects us.
    let prefix = r#"{"schema_version":1,"tools":["#;
    let suffix = "]}";
    let padding_target = 6 * 1024 * 1024; // 6 MiB > 5 MiB cap
    let entry = r#"{"name":"x","path":"tools/x.toml","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}"#;
    // Pre-size for prefix+padding+entry-overshoot+suffix so the final
    // push_str doesn't trigger a re-grow (was a 12 MiB realloc spike).
    let mut body = String::with_capacity(padding_target + entry.len() + suffix.len() + 16);
    body.push_str(prefix);
    body.push_str(entry);
    while body.len() < padding_target {
        body.push(',');
        body.push_str(entry);
    }
    body.push_str(suffix);
    let body_len = body.len();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(body));
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(EXIT_NETWORK_TIMEOUT),
        "oversized manifest must surface as fetch failure (exit \
         {EXIT_NETWORK_TIMEOUT}); got {:?}, body_len was {body_len}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Two acceptable shapes:
    // 1. Ideal: ureq's bounded read fires `FetchError::TooLarge` once
    //    it pulls cap+1 bytes off the wire ("too large" / "capped").
    // 2. Acceptable: the mock server's `write_all` errors mid-stream
    //    once the client disconnects, and ureq surfaces this as a
    //    `Read` error before TooLarge has a chance to fire ("peer
    //    disconnected" / "read error"). Either way the sync aborts and
    //    no canonical manifest is promoted — that is the real
    //    invariant.
    let lower = stderr.to_lowercase();
    assert!(
        lower.contains("too large")
            || lower.contains("capped")
            || lower.contains("peer disconnected")
            || lower.contains("read error"),
        "stderr must explain the failure; got:\n{stderr}"
    );

    // Cache must NOT contain a promoted manifest — partial download
    // must not poison subsequent syncs.
    assert!(!env.cache_dir().join("manifest.json").exists());
}

// ===== Oversized tool TOML: MAX_TOOL_BYTES = 1 MiB =====

#[test]
#[serial(registry_env)]
fn oversized_tool_body_is_rejected_no_partial_cache() {
    let env = TestEnv::new();
    // Manifest claims the sha for a >1MB body. The sha check will never
    // even run — the response cap fires first.
    let huge_body = vec![b'x'; 2 * 1024 * 1024];
    let claimed_sha = sha256_hex(&huge_body);
    let manifest_body = serde_json::json!({
        "schema_version": 1,
        "tools": [{
            "name": "huge",
            "path": "tools/huge.toml",
            "sha256": claimed_sha,
        }],
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest_body));
    routes.insert("/tools/huge.toml".to_string(), Canned::ok(huge_body));
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "oversized tool body must fail sync; got exit {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    // No leftover huge.toml in the staging or tools dir.
    let staging = env.cache_dir().join("tools.new");
    assert!(
        !staging.join("huge.toml").exists(),
        "staging must not retain oversized body"
    );
    assert!(
        !env.tools_dir().join("huge.toml").exists(),
        "tools/ must not retain oversized body"
    );
}

// ===== Truncated response body: connection drops mid-stream =====

#[test]
#[serial(registry_env)]
fn truncated_manifest_response_surfaces_read_error() {
    let env = TestEnv::new();
    let manifest_body = manifest_with(&["alpha"]);
    let truncate_at = manifest_body.len() / 2;
    let mut routes = HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        Canned::ok(manifest_body.clone()).truncated(truncate_at),
    );
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "truncated manifest must fail; got exit {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    // Truncation can surface as a Read error (incomplete chunk read by
    // ureq) OR as a Manifest parse error (the partial JSON parsed
    // far enough to look like valid JSON but is truncated). Pin one of
    // those shapes on stderr — a regression that silently parsed
    // half-JSON as `tools_synced: 0` would otherwise satisfy the
    // cache-state invariant below but pass this test.
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("read error")
            || stderr.contains("manifest")
            || stderr.contains("fetch failed")
            || stderr.contains("peer disconnected"),
        "stderr must surface a read/parse/fetch failure; got:\n{stderr}"
    );

    // Cache-state invariant: no canonical promotion regardless of
    // which error shape fired.
    assert!(!env.cache_dir().join("manifest.json").exists());
}

// ===== HTTP 500 on manifest =====

#[test]
#[serial(registry_env)]
fn manifest_http_500_maps_to_network_error() {
    let env = TestEnv::new();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::server_error());
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(EXIT_NETWORK_TIMEOUT),
        "HTTP 500 must map to exit {EXIT_NETWORK_TIMEOUT}; got {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("500"),
        "stderr must include the HTTP status; got:\n{stderr}"
    );
}

// ===== HTTP 500 on one tool of many: whole sync aborts, no partial
// canonical promotion =====

#[test]
#[serial(registry_env)]
fn one_tool_500_aborts_sync_without_promoting_others() {
    let env = TestEnv::new();
    // 3 tools in manifest; one returns 500.
    let mut routes = happy_routes(&["alpha", "beta", "gamma"]);
    routes.insert("/tools/beta.toml".to_string(), Canned::server_error());
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "partial tool failure must abort sync; got exit {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    // Atomic-swap invariant: tools/ must be empty on first-time failure.
    // No promotion of the alpha/gamma fetches even though they succeeded.
    let tools_dir = env.tools_dir();
    assert!(
        !tools_dir.join("alpha.toml").exists(),
        "alpha must NOT be promoted on partial failure"
    );
    assert!(
        !tools_dir.join("beta.toml").exists(),
        "beta MUST NOT be promoted (it 500'd)"
    );
    assert!(
        !tools_dir.join("gamma.toml").exists(),
        "gamma must NOT be promoted on partial failure"
    );
}

// ===== Many tools parallel: stress the worker pool =====

#[test]
#[serial(registry_env)]
fn many_tools_parallel_fetch_succeeds() {
    let env = TestEnv::new();
    // 32 tools — exercises max_parallel cap (8) plus the dispatch loop.
    let names: Vec<String> = (0..32).map(|i| format!("tool-{i:02}")).collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let server = MockRegistry::start(happy_routes(&name_refs));
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        out.status.success(),
        "32-tool sync must succeed; exit={:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Tools synced:     32"),
        "synced count must be 32; got:\n{stdout}"
    );
    for n in &names {
        let p = env.tools_dir().join(format!("{n}.toml"));
        assert!(p.exists(), "missing {}", p.display());
    }
    // Server got hit exactly once per tool — no retries, no duplicates.
    for n in &names {
        let path = format!("/tools/{n}.toml");
        assert_eq!(
            server.hits_for(&path),
            1,
            "{path} hit {} times, want 1",
            server.hits_for(&path)
        );
    }
}

// ===== Recovery after failure: second sync cleans up stale staging =====

#[test]
#[serial(registry_env)]
fn second_sync_recovers_from_prior_failed_sync() {
    let env = TestEnv::new();

    // First sync: a tool returns 500, so sync fails. Staging artifacts
    // may linger — that's exactly what the second sync needs to clean up.
    let mut bad_routes = happy_routes(&["alpha"]);
    bad_routes.insert("/tools/alpha.toml".to_string(), Canned::server_error());
    let bad_server = MockRegistry::start(bad_routes);
    env.write_registry_config(&bad_server.base_url, false);
    let first = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(!first.status.success(), "first sync should fail");
    drop(bad_server);

    // Second sync: server now healthy. Must clean any prior `.unverified`
    // / staging and produce a clean canonical cache.
    let good_server = MockRegistry::start(happy_routes(&["alpha"]));
    env.write_registry_config(&good_server.base_url, false);
    let second = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        second.status.success(),
        "second sync against healthy server must succeed; stderr:\n{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(env.tools_dir().join("alpha.toml").exists());
    assert!(env.cache_dir().join("manifest.json").exists());
    // No stale `.unverified` files left from either run.
    assert!(!env.cache_dir().join("manifest.json.unverified").exists());
    assert!(
        !env.cache_dir()
            .join("manifest.json.unverified.sig")
            .exists()
    );
}

// ===== Malformed JSON manifest =====

#[test]
#[serial(registry_env)]
fn malformed_json_manifest_surfaces_parse_error() {
    let env = TestEnv::new();
    let mut routes = HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        Canned::ok(b"this is not json {{{".to_vec()),
    );
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "malformed JSON must fail sync; got exit {:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("manifest")
            || stderr.to_lowercase().contains("json")
            || stderr.to_lowercase().contains("parse"),
        "stderr should mention manifest/json/parse failure; got:\n{stderr}"
    );
}

// ===== Duplicate tool names: must be rejected at parse =====
//
// Two manifest entries with the same name would have two parallel
// workers writing to the same staging file. We reject at manifest
// parse so the race never has a chance to materialize.

#[test]
#[serial(registry_env)]
fn duplicate_tool_names_in_manifest_rejected() {
    let env = TestEnv::new();
    let body_a = tool_toml("dup");
    let body_b = tool_toml("dup");
    let manifest_body = serde_json::json!({
        "schema_version": 1,
        "tools": [
            {"name": "dup", "path": "tools/dup-a.toml", "sha256": sha256_hex(&body_a)},
            {"name": "dup", "path": "tools/dup-b.toml", "sha256": sha256_hex(&body_b)},
        ],
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest_body));
    routes.insert("/tools/dup-a.toml".to_string(), Canned::ok(body_a));
    routes.insert("/tools/dup-b.toml".to_string(), Canned::ok(body_b));
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(EXIT_CONFIG_ERROR),
        "duplicate-name manifest must exit {EXIT_CONFIG_ERROR}; got {:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("dup") || stderr.to_lowercase().contains("duplicate"),
        "stderr should name the duplicate; got:\n{stderr}"
    );
}

// ===== Manifest with NO tools: succeeds with synced=0 =====
//
// Edge case: an empty registry is valid. Sync should succeed and the
// loader should observe zero plugin tools. Pins behavior so a future
// "manifest must have at least one tool" check doesn't silently break.

#[test]
#[serial(registry_env)]
fn empty_manifest_is_valid_zero_tools_synced() {
    let env = TestEnv::new();
    let manifest_body = serde_json::json!({"schema_version": 1, "tools": []}).to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest_body));
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        out.status.success(),
        "empty manifest must be a valid sync; exit={:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Tools synced:     0"),
        "synced count must be 0; got:\n{stdout}"
    );
    assert!(env.cache_dir().join("manifest.json").exists());
}

// ===== Schema-version skew rejected with actionable message =====

#[test]
#[serial(registry_env)]
fn manifest_with_future_schema_version_rejected() {
    let env = TestEnv::new();
    // schema_version = 9999 — way above whatever the binary knows.
    let manifest_body = serde_json::json!({"schema_version": 9999, "tools": []}).to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest_body));
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "future schema_version must fail sync; got exit {:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("schema") || stderr.to_lowercase().contains("version"),
        "stderr should explain the schema mismatch; got:\n{stderr}"
    );
}

// ===== ToolParseFailed: manifest sha matches an unparseable TOML body =====
//
// The orchestrator catches a body whose sha matches the manifest but
// whose TOML body is invalid. Without this test, a regression that
// swapped `toml::from_str::<PluginTool>` for a permissive parser, or
// dropped the parse check before write, would land staging bytes that
// the loader later refuses — leaving the user with an unloadable tool.

#[test]
#[serial(registry_env)]
fn manifest_sha_matches_unparseable_toml_body_is_rejected() {
    let env = TestEnv::new();
    let body = b"this is not toml: { unclosed bracket";
    let claimed_sha = sha256_hex(body);
    let manifest_body = serde_json::json!({
        "schema_version": 1,
        "tools": [{
            "name": "broken",
            "path": "tools/broken.toml",
            "sha256": claimed_sha,
        }],
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest_body));
    routes.insert("/tools/broken.toml".to_string(), Canned::ok(body.to_vec()));
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "unparseable TOML body must fail sync; got exit {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("parse") || stderr.contains("toml") || stderr.contains("broken"),
        "stderr must explain the parse failure; got:\n{stderr}"
    );

    // No partial promotion: the canonical tools dir must not hold the
    // broken file.
    assert!(
        !env.tools_dir().join("broken.toml").exists(),
        "tools/broken.toml must NOT be promoted after parse failure"
    );
}

// ===== InvalidEncoding: manifest fetch returns non-UTF-8 bytes =====
//
// A regression that piped Vec<u8> directly to serde_json (which would
// accept some non-UTF-8 garbage paths via lossy decode) would silently
// drop this validation. Pins the contract that manifest bytes must be
// valid UTF-8 before parsing.

#[test]
#[serial(registry_env)]
fn manifest_with_invalid_utf8_rejected() {
    let env = TestEnv::new();
    let mut routes = HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        // 0xFE/0xFF are invalid first bytes of any UTF-8 sequence.
        Canned::ok(vec![0xFF, 0xFE, 0xFD, 0xFC, b'\n', b'?']),
    );
    let server = MockRegistry::start(routes);
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "invalid-UTF-8 manifest must fail sync; got exit {:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("encoding") || stderr.contains("utf") || stderr.contains("manifest"),
        "stderr must explain the encoding failure; got:\n{stderr}"
    );
    assert!(!env.cache_dir().join("manifest.json").exists());
}

// ===== JARVY_REGISTRY_SYNC_PARALLELISM env-var =====
//
// The clamp `max(1).min(total.max(1)).min(64)` at sync.rs:285-290 has
// three documented behaviors:
//   1. Values below 1 clamp to 1.
//   2. Never spawn more workers than tools to fetch.
//   3. Absolute upper bound of 64 (a hostile manifest can't run us out
//      of OS threads).
//
// A robust observation of *actual* worker concurrency requires
// instrumenting the production code — out of scope here. What this
// test pins is the regression we *can* catch from outside: that the
// env var is honored, that values across the supported range all run
// the sync to a clean success, and that each tool is fetched exactly
// once regardless of the pool size. A regression that drops the
// env-var lookup or breaks the clamp boundary cases would surface as
// a panic, a hang, or duplicate fetches.

#[test]
#[serial(registry_env)]
fn parallelism_env_var_values_run_to_clean_success() {
    // (env value, tool count). Includes:
    //   "1"  — degenerate serial worker
    //   "4"  — sub-default
    //   "9999" — above the 64-cap → must clamp internally
    let cases: &[(&str, usize)] = &[("1", 4), ("4", 6), ("9999", 6)];

    for (env_value, total) in cases {
        let mut env_outer = TestEnv::new();
        env_outer.set("JARVY_REGISTRY_SYNC_PARALLELISM", env_value.into());

        let names: Vec<String> = (0..*total).map(|i| format!("clamp-tool-{i:02}")).collect();
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let server = MockRegistry::start(happy_routes(&name_refs));
        env_outer.write_registry_config(&server.base_url, false);

        let out = jarvy(&env_outer)
            .arg("registry")
            .arg("sync")
            .output()
            .expect("spawn jarvy");
        assert!(
            out.status.success(),
            "PARALLELISM={env_value} (total={total}) must succeed; stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );

        // Each tool fetched exactly once — proves the worker pool
        // dispatched the full work set without duplicate fetches under
        // every clamp value.
        for name in &names {
            assert_eq!(
                server.hits_for(&format!("/tools/{name}.toml")),
                1,
                "PARALLELISM={env_value}: tool {name} should be fetched once"
            );
            assert!(env_outer.tools_dir().join(format!("{name}.toml")).exists());
        }
    }
}
