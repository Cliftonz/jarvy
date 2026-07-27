//! Lib-visible sink for freshness-advisory metric emissions.
//!
//! `src/telemetry.rs` is bin-only (declared in `main.rs`), so the
//! checker — which lives under `lib.rs` — can't reach its `Counter`
//! / `Histogram` handles directly. Mirrors the pattern from
//! [`super::telemetry_gate`]: the bin-side `telemetry::init`
//! registers a real emitter at startup, and lib-side callers
//! (`src/maintenance/checker.rs`) invoke the sink without knowing
//! whether OTLP is wired up.
//!
//! Rationale: the observability review flagged that backend failures
//! and per-probe latency were invisible to OTLP (P0 obs F1 + F2). The
//! `tracing::warn!` event fires from the lib as before — that piece
//! didn't need this seam — but the actual `Counter::add` /
//! `Histogram::record` calls have to originate somewhere the OTel
//! meter handles live, i.e. the bin.

use std::sync::OnceLock;
use std::time::Duration;

/// Signature for the bin-side emitter that translates
/// `(backend, error_kind, duration)` into a counter bump + a
/// histogram record. `backend` and `error_kind` are `&'static str`
/// by contract so OTLP label cardinality stays bounded.
pub type BackendFailedFn = fn(backend: &'static str, error_kind: &'static str, duration: Duration);

/// Signature for the successful-probe emitter — histogram only.
pub type BackendOkFn = fn(backend: &'static str, duration: Duration);

static BACKEND_FAILED: OnceLock<BackendFailedFn> = OnceLock::new();
static BACKEND_OK: OnceLock<BackendOkFn> = OnceLock::new();

/// Install the failure emitter. Called once from `telemetry::init`
/// after the OTel meter provider is built. Idempotent — the first
/// `set` wins.
pub fn set_backend_failed_emitter(f: BackendFailedFn) {
    let _ = BACKEND_FAILED.set(f);
}

/// Install the success emitter.
pub fn set_backend_ok_emitter(f: BackendOkFn) {
    let _ = BACKEND_OK.set(f);
}

/// Emit a backend-failure counter + duration record. No-op when
/// the bin hasn't wired up an emitter (typical during `cargo test`
/// against the lib crate) or when the telemetry gate is off.
pub fn backend_failed(backend: &'static str, error_kind: &'static str, duration: Duration) {
    if !super::telemetry_gate::is_enabled() {
        return;
    }
    if let Some(f) = BACKEND_FAILED.get() {
        f(backend, error_kind, duration);
    }
}

/// Emit a successful-probe duration record.
pub fn backend_ok(backend: &'static str, duration: Duration) {
    if !super::telemetry_gate::is_enabled() {
        return;
    }
    if let Some(f) = BACKEND_OK.get() {
        f(backend, duration);
    }
}
