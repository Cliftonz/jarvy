//! End-to-end tests for the cosign signature path of `jarvy registry sync`.
//!
//! Exercises `require_signature = true` flows via the real `jarvy`
//! binary, the in-process `MockRegistry`, and the `FakeCosign` shim.
//!
//! Coverage:
//! - Happy path: cosign exits 0 → sync succeeds, sig/pem promoted to
//!   canonical paths.
//! - Cosign rejects → sync exits config-error, stderr names the rejection,
//!   cache holds no canonical manifest.
//! - Cosign missing from PATH → sync exits config-error with the
//!   "install cosign" hint.
//! - Server omits `.sig` (404) → sync exits network-error with no
//!   canonical files written.
//!
//! Each test runs jarvy as a child process so `cosign_on_path()`'s
//! process-wide `OnceLock` cache doesn't leak between cases.

#![allow(unsafe_code)] // env mutation fenced by #[serial(registry_env)]

mod common;

use std::collections::HashMap;

use serial_test::serial;

use common::registry::{
    Canned, EXIT_CONFIG_ERROR, EXIT_NETWORK_TIMEOUT, FakeCosign, MockRegistry, TestEnv,
    happy_routes, jarvy_cmd as jarvy, manifest_with, tool_toml,
};

/// Anchored regex that matches the FakeCosign shim's permissive output.
/// The shim exits 0 regardless of identity, but `Config::validate`
/// refuses unanchored regex, so we still must supply one anchored
/// pattern. The actual identity check happens inside cosign — when we
/// fake-pass, the regex is essentially decoration.
const FAKE_IDENTITY_REGEXP: &str =
    r"^https://github\.com/test/.*@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$";

/// Convenience: write a `[registry]` config with require_signature=true
/// plus the FakeCosign-friendly identity regex. Uses TOML literal-string
/// syntax (single quotes) for the regex so backslashes inside it
/// survive parsing without escaping.
fn write_signed_config(env: &TestEnv, base_url: &str) {
    let body = format!(
        r#"
[registry]
url = "{base_url}"
enabled = true
require_signature = true
signature_identity_regexp = '{FAKE_IDENTITY_REGEXP}'
"#
    );
    std::fs::write(env.config_path(), body).expect("write config");
}

/// Extend the happy-path routes with the cosign companion files. The
/// signature/cert bodies don't have to be real — the FakeCosign shim
/// only checks its exit-code env vars. We just need the server to
/// answer 200 on /manifest.json.sig and /manifest.json.pem.
fn signed_happy_routes(tools: &[&str]) -> HashMap<String, Canned> {
    let mut routes = happy_routes(tools);
    routes.insert(
        "/manifest.json.sig".to_string(),
        Canned::ok(b"fake-signature".to_vec()),
    );
    routes.insert(
        "/manifest.json.pem".to_string(),
        Canned::ok(b"fake-cert-pem".to_vec()),
    );
    routes
}

// ===== Happy path: cosign verifies, sync succeeds =====

#[test]
#[serial(registry_env)]
fn signed_sync_promotes_sig_and_pem_on_verify_success() {
    let mut env = TestEnv::new();
    let cosign = FakeCosign::new();
    env.prepend_path(cosign.dir());
    // Force the shim to exit 0 explicitly so a leaked truthy env from a
    // sibling test can't flip us into the rejection path.
    env.set("FAKE_COSIGN_VERIFY_EXIT", "0".into());
    env.remove("FAKE_COSIGN_STDERR");

    let server = MockRegistry::start(signed_happy_routes(&["alpha"]));
    write_signed_config(&env, &server.base_url);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        out.status.success(),
        "signed sync must succeed; exit={:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("verified (cosign)"),
        "signature line must read 'verified (cosign)'; got:\n{stdout}"
    );

    // Canonical sig + pem files promoted, no .unverified leftovers.
    let cache = env.cache_dir();
    assert!(
        cache.join("manifest.json").exists(),
        "manifest.json missing"
    );
    assert!(cache.join("manifest.json.sig").exists(), ".sig missing");
    assert!(cache.join("manifest.json.pem").exists(), ".pem missing");
    assert!(
        !cache.join("manifest.json.unverified").exists(),
        "staging .unverified must be cleaned up on success"
    );
    // Post-fix staging shape: `verify_sigstore_signature_with_identity`
    // derives siblings via `with_extension("unverified.sig")` on the
    // `manifest.json.unverified` input, so cosign reads (and we stage)
    // `manifest.json.unverified.{sig,pem}`. A revert of sync.rs:180-181
    // to the pre-fix `.sig.unverified` shape would leave attacker bytes
    // at the new path with no test catching it — these two assertions
    // are the regression guard.
    assert!(
        !cache.join("manifest.json.unverified.sig").exists(),
        ".unverified.sig must be cleaned up on success"
    );
    assert!(
        !cache.join("manifest.json.unverified.pem").exists(),
        ".unverified.pem must be cleaned up on success"
    );

    // Cosign was actually invoked (the shim ran). The sig + pem URLs
    // were fetched, proving the require_signature path executed.
    assert_eq!(
        server.hits_for("/manifest.json.sig"),
        1,
        ".sig must be fetched exactly once"
    );
    assert_eq!(
        server.hits_for("/manifest.json.pem"),
        1,
        ".pem must be fetched exactly once"
    );

    // meta.json carries `signature_verified: true` so `jarvy registry
    // status` (which dumps meta.json verbatim) reports the right state
    // to fleet operators. Regression guard: a refactor that fed `false`
    // into meta on the verified path would still print "verified
    // (cosign)" on stdout but contradict it in the status dump.
    let meta_raw = std::fs::read_to_string(cache.join("meta.json"))
        .expect("meta.json must exist after successful sync");
    let meta: serde_json::Value =
        serde_json::from_str(&meta_raw).expect("meta.json must be valid JSON");
    assert_eq!(
        meta["signature_verified"], true,
        "meta.json must record signature_verified=true on the cosign-verified path; got:\n{meta_raw}"
    );
    assert_eq!(
        meta["tools_count"], 1,
        "meta.json must record the synced tool count; got:\n{meta_raw}"
    );
}

// ===== Cosign rejects: sync fails, no canonical files written =====

#[test]
#[serial(registry_env)]
fn signed_sync_rejects_when_cosign_exits_nonzero() {
    let mut env = TestEnv::new();
    let cosign = FakeCosign::new();
    env.prepend_path(cosign.dir());
    env.set("FAKE_COSIGN_VERIFY_EXIT", "1".into());
    env.set(
        "FAKE_COSIGN_STDERR",
        "error: certificate identity did not match".into(),
    );

    let server = MockRegistry::start(signed_happy_routes(&["alpha"]));
    write_signed_config(&env, &server.base_url);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(EXIT_CONFIG_ERROR),
        "rejected signature must map to exit {EXIT_CONFIG_ERROR}; got {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("sigstore")
            || stderr.to_lowercase().contains("signature")
            || stderr.to_lowercase().contains("reject"),
        "stderr must surface the signature failure; got:\n{stderr}"
    );

    // Canonical manifest must NOT exist — the prior known-good triplet
    // invariant (or empty cache on first sync) must hold.
    let cache = env.cache_dir();
    assert!(
        !cache.join("manifest.json").exists(),
        "rejected sync must not promote manifest.json"
    );
    assert!(
        !cache.join("manifest.json.sig").exists(),
        "rejected sync must not promote .sig"
    );
    assert!(
        !cache.join("manifest.json.pem").exists(),
        "rejected sync must not promote .pem"
    );
    // And the staging `.unverified` files must be cleaned up — attacker
    // bytes must not linger.
    assert!(
        !cache.join("manifest.json.unverified").exists(),
        "rejected sync must clean up manifest.json.unverified"
    );
    // Post-fix staging shape (see corresponding comment in the happy
    // path above). The cleanup at sync.rs:203-205 must wipe the bytes
    // cosign just rejected; otherwise an attacker manifest lingers
    // readable on disk until the next sync.
    assert!(
        !cache.join("manifest.json.unverified.sig").exists(),
        "rejected sync must clean up .unverified.sig"
    );
    assert!(
        !cache.join("manifest.json.unverified.pem").exists(),
        "rejected sync must clean up .unverified.pem"
    );

    // Tool files must NOT be promoted either — the per-tool fetch loop
    // runs AFTER signature verification.
    assert!(
        !env.tools_dir().join("alpha.toml").exists(),
        "rejected sync must not write any tool TOMLs"
    );
}

// ===== Cosign missing from PATH: clear hint, fail closed =====

#[test]
#[serial(registry_env)]
fn signed_sync_fails_when_cosign_not_on_path() {
    let mut env = TestEnv::new();
    // Override PATH to a directory we know has no cosign binary. The
    // empty TempDir we create here is our isolated PATH.
    let empty_dir = tempfile::TempDir::new().expect("empty path dir");
    env.set("PATH", empty_dir.path().as_os_str().to_os_string());

    let server = MockRegistry::start(signed_happy_routes(&["alpha"]));
    write_signed_config(&env, &server.base_url);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(EXIT_CONFIG_ERROR),
        "missing cosign must exit {EXIT_CONFIG_ERROR}; got {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cosign") && stderr.contains("install"),
        "stderr must tell the user to install cosign; got:\n{stderr}"
    );

    let cache = env.cache_dir();
    assert!(
        !cache.join("manifest.json").exists(),
        "cosign-missing sync must not promote manifest"
    );
}

// ===== Server omits the .sig: surfaces fetch failure cleanly =====

#[test]
#[serial(registry_env)]
fn signed_sync_fails_when_sig_file_missing_on_server() {
    let mut env = TestEnv::new();
    let cosign = FakeCosign::new();
    env.prepend_path(cosign.dir());
    env.set("FAKE_COSIGN_VERIFY_EXIT", "0".into());

    // Serve manifest + tool but NOT the .sig — server returns 404.
    let mut routes = happy_routes(&["alpha"]);
    routes.insert(
        "/manifest.json.pem".to_string(),
        Canned::ok(b"fake-cert-pem".to_vec()),
    );
    // /manifest.json.sig intentionally absent — falls through to 404.
    let server = MockRegistry::start(routes);
    write_signed_config(&env, &server.base_url);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(EXIT_NETWORK_TIMEOUT),
        "missing .sig is a fetch failure (404 = NETWORK_TIMEOUT); got {:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let cache = env.cache_dir();
    assert!(
        !cache.join("manifest.json").exists(),
        "missing-sig sync must not promote manifest"
    );
}

// ===== Disabled signature path still works (escape hatch) =====
//
// Locks in the `require_signature = false` opt-out: warning on stderr,
// successful sync, no cosign invocation needed. This is the
// documented loopback-mirror / local-dev path.

#[test]
#[serial(registry_env)]
fn unsigned_sync_warns_on_stderr_but_succeeds() {
    let mut env = TestEnv::new();
    // No FakeCosign — proves cosign is never called when
    // require_signature=false.
    let empty_dir = tempfile::TempDir::new().expect("empty path dir");
    env.set("PATH", empty_dir.path().as_os_str().to_os_string());

    // happy_routes() doesn't add sig/pem, which is fine — they're never
    // fetched on the unsigned path.
    let server = MockRegistry::start(happy_routes(&["alpha"]));
    env.write_registry_config(&server.base_url, false);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        out.status.success(),
        "unsigned sync must succeed; exit={:?}, stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("WARNING") && stderr.contains("require_signature=false"),
        "unsigned sync must emit the documented stderr warning; got:\n{stderr}"
    );

    assert_eq!(
        server.hits_for("/manifest.json.sig"),
        0,
        "unsigned sync must NOT fetch the .sig"
    );
    assert_eq!(
        server.hits_for("/manifest.json.pem"),
        0,
        "unsigned sync must NOT fetch the .pem"
    );

    let cache = env.cache_dir();
    assert!(cache.join("manifest.json").exists());
    assert!(env.tools_dir().join("alpha.toml").exists());
}

// ===== Manifest body mismatch with sig still rejects (covers the
// "right cosign output but wrong content under it" interleave) =====
//
// FakeCosign exits 0 regardless of what's under it, so this test
// instead verifies the sha-mismatch check on the *tool* file fires
// even when the manifest signature path succeeds. It's a guard that
// signature verification doesn't accidentally short-circuit the
// per-tool integrity check downstream.

#[test]
#[serial(registry_env)]
fn signed_sync_still_enforces_per_tool_sha_after_verify() {
    let mut env = TestEnv::new();
    let cosign = FakeCosign::new();
    env.prepend_path(cosign.dir());
    env.set("FAKE_COSIGN_VERIFY_EXIT", "0".into());

    // Manifest claims alpha.toml's sha matches `tool_toml("alpha")`, but
    // the server serves a DIFFERENT body for that path. The sig-verify
    // step succeeds (because the FakeCosign shim greenlights it), but
    // the per-tool sha check must still fail.
    let mut routes = signed_happy_routes(&["alpha"]);
    routes.insert(
        "/tools/alpha.toml".to_string(),
        Canned::ok(b"completely-different-bytes".to_vec()),
    );
    let server = MockRegistry::start(routes);
    write_signed_config(&env, &server.base_url);

    let out = jarvy(&env).arg("registry").arg("sync").output().unwrap();
    assert!(
        !out.status.success(),
        "tampered tool body must fail sync even when signature verifies; \
         exit={:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("sha")
            || stderr.to_lowercase().contains("checksum")
            || stderr.to_lowercase().contains("mismatch"),
        "stderr must explain the sha mismatch; got:\n{stderr}"
    );

    // Confirms `tool_toml` and `manifest_with` agree (defensive — if
    // this assert ever flips, the test isn't actually testing tampering).
    let canonical = tool_toml("alpha");
    let parsed: serde_json::Value =
        serde_json::from_str(&manifest_with(&["alpha"])).expect("manifest parses");
    let claimed_sha = parsed["tools"][0]["sha256"].as_str().unwrap().to_string();
    let actual_sha = common::registry::sha256_hex(&canonical);
    assert_eq!(
        claimed_sha, actual_sha,
        "manifest_with() must encode the sha of tool_toml(); fixtures drifted"
    );
}
