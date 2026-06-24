//! Tracing-event regression guards for `registry_remote::sync`.
//!
//! Every other registry test runs the binary with `JARVY_TELEMETRY=0`,
//! which gates off every `emit(|| tracing::*)` call in `sync.rs`. That
//! makes the documented OTEL event taxonomy (`registry.sync.*`,
//! `registry.cache.*`, etc.) impossible to assert through the CLI
//! surface. These tests instead call `run_sync_with_config` in-process
//! against a tracing subscriber that captures every `event=` field, so
//! a regression that renames an event, drops a field, or downgrades a
//! level can be caught by name.
//!
//! Without this, dashboards and alerts keyed on `event =
//! "registry.sync.sha_mismatch"` (etc.) silently zero out under
//! incident conditions.

#![allow(unsafe_code)] // env mutation fenced by #[serial(registry_env)]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serial_test::serial;
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;

use common::registry::{Canned, MockRegistry, TestEnv, happy_routes, sha256_hex, tool_toml};

// ===== Tracing capture layer =====

#[derive(Debug, Clone)]
struct Captured {
    event: String,
    level: Level,
    fields: HashMap<String, String>,
}

#[derive(Default, Clone)]
struct CaptureLayer {
    inner: Arc<Mutex<Vec<Captured>>>,
}

// (CaptureLayer holds the events; `capture()` snapshots the inner Vec
// after the closure returns. No separate accessor needed.)

struct FieldGrab {
    out: HashMap<String, String>,
}

impl Visit for FieldGrab {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.out
            .insert(field.name().to_string(), format!("{value:?}"));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.out.insert(field.name().to_string(), value.to_string());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.out.insert(field.name().to_string(), value.to_string());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.out.insert(field.name().to_string(), value.to_string());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.out.insert(field.name().to_string(), value.to_string());
    }
}

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn enabled(&self, _meta: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        true
    }
    fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut grab = FieldGrab {
            out: HashMap::new(),
        };
        event.record(&mut grab);
        if let Some(event_name) = grab.out.get("event").cloned() {
            self.inner.lock().unwrap().push(Captured {
                event: event_name,
                level: *event.metadata().level(),
                fields: grab.out,
            });
        }
    }
}

/// Process-wide capture sink. `run_sync_with_config` spawns worker
/// threads via `std::thread::scope`, and tracing's thread-local
/// default subscriber doesn't propagate into spawned threads — so a
/// `with_default(...)` scope can't see worker events. A global
/// subscriber installed once for the test binary's lifetime catches
/// them all. `#[serial(registry_env)]` ensures tests don't interleave
/// captures.
static CAPTURE: OnceLock<CaptureLayer> = OnceLock::new();

fn init_capture() -> &'static CaptureLayer {
    CAPTURE.get_or_init(|| {
        let layer = CaptureLayer::default();
        tracing_subscriber::registry().with(layer.clone()).init();
        layer
    })
}

/// Run `f` with the capture sink active and snapshot any events that
/// fired during the call. The sink is reset before `f` runs.
fn capture<F: FnOnce()>(f: F) -> Vec<Captured> {
    let layer = init_capture();
    layer.inner.lock().unwrap().clear();
    f();
    layer.inner.lock().unwrap().clone()
}

/// Toggle the process-wide telemetry gate so the `emit()` wrapper
/// actually fires events. RAII guard restores the prior value on drop.
struct GateGuard {
    prior: bool,
}

impl GateGuard {
    fn on() -> Self {
        let prior = jarvy::observability::telemetry_gate::is_enabled();
        jarvy::observability::telemetry_gate::set_enabled(true);
        Self { prior }
    }
}

impl Drop for GateGuard {
    fn drop(&mut self) {
        jarvy::observability::telemetry_gate::set_enabled(self.prior);
    }
}

fn cfg_for(env: &TestEnv, base_url: &str) -> jarvy::registry_remote::RegistryConfig {
    let _ = env;
    jarvy::registry_remote::RegistryConfig {
        url: base_url.to_string(),
        enabled: true,
        require_signature: false,
        ..Default::default()
    }
}

// ===== Happy path: completed event fires with expected fields =====

#[test]
#[serial(registry_env)]
fn happy_sync_emits_started_and_completed_events() {
    let env = TestEnv::new();
    let server = MockRegistry::start(happy_routes(&["alpha"]));
    let cfg = cfg_for(&env, &server.base_url);

    let events = capture(|| {
        let _gate = GateGuard::on();
        let _ = jarvy::registry_remote::sync::run_sync_with_config(&cfg);
    });

    let started = events
        .iter()
        .find(|e| e.event == "registry.sync.started")
        .expect("registry.sync.started must fire");
    assert_eq!(started.level, Level::INFO);
    assert!(
        started.fields.contains_key("registry_url"),
        "registry.sync.started must carry registry_url; got fields {:?}",
        started.fields
    );

    let completed = events
        .iter()
        .find(|e| e.event == "registry.sync.completed")
        .expect("registry.sync.completed must fire");
    assert_eq!(completed.level, Level::INFO);
    for required in [
        "registry_url",
        "tools_synced",
        "tools_removed",
        "duration_ms",
    ] {
        assert!(
            completed.fields.contains_key(required),
            "registry.sync.completed must carry `{required}`; got fields {:?}",
            completed.fields
        );
    }
}

// ===== Sha mismatch fires the error-level event with tool + url =====

#[test]
#[serial(registry_env)]
fn sha_mismatch_emits_structured_error_event() {
    let env = TestEnv::new();
    let real_body = tool_toml("mismatched");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "tools": [{
            "name": "mismatched",
            "path": "tools/mismatched.toml",
            "sha256": sha256_hex(&real_body),
        }],
    })
    .to_string();
    let mut routes = HashMap::new();
    routes.insert("/manifest.json".to_string(), Canned::ok(manifest));
    // Tampered body — sha won't match.
    routes.insert(
        "/tools/mismatched.toml".to_string(),
        Canned::ok(b"hostile bytes".to_vec()),
    );
    let server = MockRegistry::start(routes);
    let cfg = cfg_for(&env, &server.base_url);

    let events = capture(|| {
        let _gate = GateGuard::on();
        let _ = jarvy::registry_remote::sync::run_sync_with_config(&cfg);
    });

    let sha = events
        .iter()
        .find(|e| e.event == "registry.sync.sha_mismatch")
        .expect("registry.sync.sha_mismatch must fire on tampered body");
    assert_eq!(sha.level, Level::ERROR);
    assert_eq!(
        sha.fields.get("tool").map(String::as_str),
        Some("mismatched"),
        "sha_mismatch event must name the bad tool; got {:?}",
        sha.fields
    );
    for required in ["expected", "actual", "url"] {
        assert!(
            sha.fields.contains_key(required),
            "sha_mismatch must carry `{required}`; got {:?}",
            sha.fields
        );
    }
}

// ===== signature_disabled fires on the require_signature=false path =====

#[test]
#[serial(registry_env)]
fn unsigned_sync_emits_signature_disabled_event() {
    let env = TestEnv::new();
    let server = MockRegistry::start(happy_routes(&["alpha"]));
    let cfg = cfg_for(&env, &server.base_url);

    let events = capture(|| {
        let _gate = GateGuard::on();
        let _ = jarvy::registry_remote::sync::run_sync_with_config(&cfg);
    });

    let disabled = events
        .iter()
        .find(|e| e.event == "registry.signature_disabled")
        .expect("registry.signature_disabled must fire when require_signature=false");
    assert_eq!(disabled.level, Level::WARN);
    assert!(
        disabled.fields.contains_key("registry_url"),
        "signature_disabled must carry registry_url"
    );
}

// ===== Manifest-parse failure emits stage=manifest_parse =====

#[test]
#[serial(registry_env)]
fn malformed_manifest_emits_failed_event_with_stage_label() {
    let env = TestEnv::new();
    let mut routes = HashMap::new();
    routes.insert(
        "/manifest.json".to_string(),
        Canned::ok(b"this is not json {{{".to_vec()),
    );
    let server = MockRegistry::start(routes);
    let cfg = cfg_for(&env, &server.base_url);

    let events = capture(|| {
        let _gate = GateGuard::on();
        let _ = jarvy::registry_remote::sync::run_sync_with_config(&cfg);
    });

    let failed = events
        .iter()
        .find(|e| e.event == "registry.sync.failed")
        .expect("registry.sync.failed must fire on parse error");
    assert_eq!(failed.level, Level::ERROR);
    assert_eq!(
        failed.fields.get("stage").map(String::as_str),
        Some("manifest_parse"),
        "stage label must be `manifest_parse`; got {:?}",
        failed.fields
    );
}
