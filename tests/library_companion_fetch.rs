//! E2E tests for library companion-file fetch (PRD-054 follow-up).
//!
//! Drives the full pipeline over loopback HTTP (test-bypass feature:
//! `JARVY_LIBRARY_ALLOW_INSECURE_FETCH` + loopback host):
//!
//! 1. `library_registry::sync` pulls a manifest whose items reference
//!    bodies by URL (`bash_url` for ai_hooks, `companion_files` for
//!    skills).
//! 2. `skills::install_skill` fetches SKILL.md + companions, verifies
//!    every sha pin, and writes the files to the agent skills dir.
//! 3. Tamper / traversal cases refuse without touching the disk.
//! 4. The content-addressed companion cache serves repeat installs
//!    without re-hitting the server.
//!
//! Manifest delivery: manifests are https-only at the URL parser (a
//! loopback `http://` manifest URL is refused, unlike the companion
//! *bodies* which the fetch layer serves over loopback http under the
//! `JARVY_LIBRARY_ALLOW_INSECURE_FETCH` bypass). So the manifest is
//! seeded straight into the disk cache and `sync()` is pointed at an
//! unreachable host — the network fetch fails and falls back to the
//! seeded cache. This is the same door `seed_manifest` uses in
//! `library_registry_integration.rs`. Only ONE mock server runs (the
//! artifact server whose port pins the body URLs).
//!
//! Gated behind `required-features = ["test-bypass"]` in Cargo.toml —
//! release builds compile the loopback bypass out entirely.

#![allow(unsafe_code)] // env mutation fenced by #[serial(registry_env)]

mod common;

use common::registry::{Canned, MockRegistry, TestEnv, sha256_hex};
use jarvy::library_registry::{
    self, LibrarySource,
    manifest::{
        LibraryHookItem, LibraryItem, LibrarySkillItem, MANIFEST_SCHEMA_VERSION, Manifest,
        SkillCompanionFile,
    },
};
use jarvy::skills::{SkillAgent, SkillEntry, SkillError, install_skill};
use serial_test::serial;
use std::collections::HashMap;

const SKILL_BODY: &[u8] = b"---\nname: companion-skill\nversion: 1.0.0\n---\n# Skill\n";
const HELPER_BODY: &[u8] = b"#!/bin/sh\necho helper\n";
const RULE_BODY: &[u8] = b"# rule body\n";
const HOOK_BODY: &[u8] = b"#!/bin/sh\necho blocked\nexit 2\n";

fn library_source(url: &str) -> LibrarySource {
    LibrarySource {
        url: url.to_string(),
        require_signature: false,
        identity_regexp: None,
        oidc_issuer: None,
        refresh_interval_secs: 86_400,
        manifest_sha256: None,
    }
}

fn artifact_routes() -> HashMap<String, Canned> {
    let mut r = HashMap::new();
    r.insert("/skills/SKILL.md".to_string(), Canned::ok(SKILL_BODY));
    r.insert("/skills/helper.sh".to_string(), Canned::ok(HELPER_BODY));
    r.insert("/skills/rules/extra.md".to_string(), Canned::ok(RULE_BODY));
    r.insert("/hooks/block.sh".to_string(), Canned::ok(HOOK_BODY));
    r
}

/// One skill (SKILL.md + two companions, one in a subdir) and one
/// ai_hook whose bash body is URL-referenced. `helper_sha` /
/// `helper_filename` let tamper / traversal tests poison one entry.
fn manifest_for(artifact_base: &str, helper_sha: &str, helper_filename: &str) -> Manifest {
    Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        publisher: "companion-e2e".into(),
        description: String::new(),
        homepage: String::new(),
        generated_at: String::new(),
        items: vec![
            LibraryItem::Skill(LibrarySkillItem {
                name: "companion-skill".into(),
                version: "1.0.0".into(),
                description: String::new(),
                skill_md_url: format!("{artifact_base}skills/SKILL.md"),
                skill_md_sha256: sha256_hex(SKILL_BODY),
                companion_files: vec![
                    SkillCompanionFile {
                        filename: helper_filename.into(),
                        url: format!("{artifact_base}skills/helper.sh"),
                        sha256: helper_sha.into(),
                    },
                    SkillCompanionFile {
                        filename: "rules/extra.md".into(),
                        url: format!("{artifact_base}skills/rules/extra.md"),
                        sha256: sha256_hex(RULE_BODY),
                    },
                ],
                supported_agents: Vec::new(),
            }),
            LibraryItem::AiHook(LibraryHookItem {
                name: "remote-body-hook".into(),
                version: "1.0.0".into(),
                description: String::new(),
                event: "pre_tool_use".into(),
                matcher: Some("Bash".into()),
                bash: None,
                bash_url: Some(format!("{artifact_base}hooks/block.sh")),
                bash_sha256: Some(sha256_hex(HOOK_BODY)),
                powershell: None,
                powershell_url: None,
                powershell_sha256: None,
                timeout_ms: 5_000,
            }),
        ],
    }
}

/// Manifest URL for the seed-then-fallback delivery. `.invalid` is an
/// RFC 2606 reserved TLD guaranteed never to resolve, so the network
/// fetch fails instantly and `sync()` falls back to the seeded cache.
const MANIFEST_URL: &str = "https://companion-e2e.invalid/manifest.json";

/// Start the artifact server, seed the manifest into the disk cache, and
/// populate the in-process resolver via `sync()`'s cache fallback.
/// Returns `(env, artifact_server)`.
fn synced_pair(helper_sha: &str, helper_filename: &str) -> (TestEnv, MockRegistry) {
    let mut env = TestEnv::new();
    // TestEnv enables the registry bypass; library fetches read their
    // own env var so test isolation stays per-subsystem. This bypass
    // covers the SKILL.md / companion / hook-body fetches over loopback
    // http — NOT the manifest, which is delivered via the cache below.
    env.set("JARVY_LIBRARY_ALLOW_INSECURE_FETCH", "1".into());
    library_registry::clear_cache();

    let artifacts = MockRegistry::start(artifact_routes());
    let manifest = manifest_for(&artifacts.base_url, helper_sha, helper_filename);

    // Seed the manifest into the disk cache, then sync against an
    // unreachable host: the fetch fails and `sync()` falls back to the
    // seeded copy, populating the in-process resolver. Mirrors
    // `seed_manifest` in library_registry_integration.rs.
    let cache_path =
        library_registry::cache::manifest_cache_path(MANIFEST_URL).expect("cache path resolvable");
    library_registry::cache::write_manifest(&cache_path, &manifest).expect("seed manifest");
    library_registry::sync(&library_source(MANIFEST_URL))
        .expect("manifest sync falls back to seeded cache");

    (env, artifacts)
}

fn claude_skill_dir(env: &TestEnv) -> std::path::PathBuf {
    env.home()
        .join(".claude")
        .join("skills")
        .join("companion-skill")
}

// ===== Happy path: skill install pulls SKILL.md + companions =====

#[test]
#[serial(registry_env)]
fn skill_install_fetches_and_writes_companion_files() {
    let (env, artifacts) = synced_pair(&sha256_hex(HELPER_BODY), "helper.sh");

    let entry = SkillEntry::Version("1.0.0".to_string());
    let result = install_skill("companion-skill", &entry, &[SkillAgent::ClaudeCode])
        .expect("install succeeds");
    assert_eq!(result.version, "1.0.0");
    assert_eq!(result.agents, vec![SkillAgent::ClaudeCode]);

    let skill_dir = claude_skill_dir(&env);
    assert_eq!(
        std::fs::read(skill_dir.join("SKILL.md")).unwrap(),
        SKILL_BODY
    );
    assert_eq!(
        std::fs::read(skill_dir.join("helper.sh")).unwrap(),
        HELPER_BODY
    );
    assert_eq!(
        std::fs::read(skill_dir.join("rules").join("extra.md")).unwrap(),
        RULE_BODY
    );

    // Sidecar records the companions for drift detection.
    let sidecar: serde_json::Value =
        serde_json::from_slice(&std::fs::read(skill_dir.join(".jarvy-skill.json")).unwrap())
            .unwrap();
    assert_eq!(sidecar["companions"].as_array().unwrap().len(), 2);

    // Each companion was fetched exactly once.
    assert_eq!(artifacts.hits_for("/skills/helper.sh"), 1);
    assert_eq!(artifacts.hits_for("/skills/rules/extra.md"), 1);

    library_registry::clear_cache();
    drop(env);
}

// ===== Content-addressed cache: repeat install skips the network =====

#[test]
#[serial(registry_env)]
fn repeat_install_serves_companions_from_content_addressed_cache() {
    let (env, artifacts) = synced_pair(&sha256_hex(HELPER_BODY), "helper.sh");

    let entry = SkillEntry::Version("1.0.0".to_string());
    install_skill("companion-skill", &entry, &[SkillAgent::ClaudeCode]).expect("first install");
    install_skill("companion-skill", &entry, &[SkillAgent::ClaudeCode]).expect("second install");

    // Companions are content-addressed by their sha pin — the second
    // install must be served from `~/.jarvy/library.d/companions/`.
    assert_eq!(
        artifacts.hits_for("/skills/helper.sh"),
        1,
        "second install must hit the companion cache, not the network"
    );
    assert_eq!(artifacts.hits_for("/skills/rules/extra.md"), 1);
    // SKILL.md itself is not content-addressed-cached — fetched per install.
    assert_eq!(artifacts.hits_for("/skills/SKILL.md"), 2);

    library_registry::clear_cache();
    drop(env);
}

// ===== Tampered companion: sha pin mismatch refuses the install =====

#[test]
#[serial(registry_env)]
fn tampered_companion_body_refuses_install() {
    // Manifest pins a sha the served body will never match.
    let wrong_sha = "c".repeat(64);
    let (env, _artifacts) = synced_pair(&wrong_sha, "helper.sh");

    let entry = SkillEntry::Version("1.0.0".to_string());
    let err = install_skill("companion-skill", &entry, &[SkillAgent::ClaudeCode])
        .expect_err("tampered companion must refuse");
    match err {
        SkillError::Companion {
            filename, source, ..
        } => {
            assert_eq!(filename, "helper.sh");
            assert_eq!(source.kind(), "sha_mismatch");
        }
        other => panic!("expected Companion sha_mismatch, got {other:?}"),
    }

    // Nothing was written — refusal happens before any agent-dir write.
    assert!(
        !claude_skill_dir(&env).exists(),
        "refused install must not leave a partial skill dir"
    );

    library_registry::clear_cache();
    drop(env);
}

// ===== Hostile filename: traversal shape refuses before any fetch =====

#[test]
#[serial(registry_env)]
fn traversal_companion_filename_refuses_install() {
    let (env, artifacts) = synced_pair(&sha256_hex(HELPER_BODY), "../../../.bashrc");

    let entry = SkillEntry::Version("1.0.0".to_string());
    let err = install_skill("companion-skill", &entry, &[SkillAgent::ClaudeCode])
        .expect_err("traversal filename must refuse");
    match err {
        SkillError::CompanionRefused { skill, .. } => assert_eq!(skill, "companion-skill"),
        other => panic!("expected CompanionRefused, got {other:?}"),
    }

    // Refusal happens before ANY companion fetch.
    assert_eq!(artifacts.hits_for("/skills/helper.sh"), 0);
    assert!(!claude_skill_dir(&env).exists());
    assert!(
        !env.home().join(".bashrc").exists(),
        "traversal target must never be written"
    );

    library_registry::clear_cache();
    drop(env);
}

// ===== ai_hook bash_url: resolve + fetch + verify over loopback =====

#[test]
#[serial(registry_env)]
fn hook_bash_url_fetches_verified_body_over_http() {
    let (env, artifacts) = synced_pair(&sha256_hex(HELPER_BODY), "helper.sh");

    let item = library_registry::resolve_hook("remote-body-hook").expect("hook in cache");
    assert!(item.bash.is_none(), "fixture hook is URL-referenced");
    let url = item.bash_url.as_deref().expect("bash_url set");
    let sha = item.bash_sha256.as_deref().expect("bash_sha256 set");

    let body = library_registry::companion::fetch_verified_utf8(url, sha)
        .expect("verified fetch succeeds");
    assert_eq!(body.as_bytes(), HOOK_BODY);
    assert_eq!(artifacts.hits_for("/hooks/block.sh"), 1);

    // Second fetch is served from the content-addressed cache.
    let again = library_registry::companion::fetch_verified_utf8(url, sha).expect("cache hit");
    assert_eq!(again.as_bytes(), HOOK_BODY);
    assert_eq!(artifacts.hits_for("/hooks/block.sh"), 1);

    // Tamper check: same URL with a non-matching pin refuses.
    let wrong = "d".repeat(64);
    let err = library_registry::companion::fetch_verified_utf8(url, &wrong)
        .expect_err("wrong pin must refuse");
    assert_eq!(err.kind(), "sha_mismatch");

    library_registry::clear_cache();
    drop(env);
}
