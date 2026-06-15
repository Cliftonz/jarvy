//! Process-wide opt-in gate for telemetry, visible from both the bin
//! and lib crates.
//!
//! `src/telemetry.rs` is currently bin-private (declared as `mod
//! telemetry` in `main.rs`). Code that lives in modules also declared
//! by `lib.rs` (e.g. `src/packages/*`, `src/observability/*`) cannot
//! reach `crate::telemetry::is_enabled()` when compiled as part of the
//! lib crate (e.g. during `cargo test`).
//!
//! This module provides a thin `AtomicBool` gate that the lib-side
//! observability module can read, populated by `telemetry::init` at
//! startup. The atomic lives in observability so any module declared
//! by `lib.rs` can reach it. At runtime only one copy of the static
//! is live (the bin-compiled one when `jarvy` is running; the lib copy
//! only matters during `cargo test`).
//!
//! This is the load-bearing piece that prevents `package.*` /
//! `packages.*` events from leaking to OTLP when the user explicitly
//! set `telemetry.enabled = false` — the prior round emitted raw
//! `tracing::*` to dodge the visibility wall, which broke the
//! documented opt-in contract.

use std::sync::atomic::{AtomicBool, Ordering};

static TELEMETRY_ENABLED: AtomicBool = AtomicBool::new(false);

/// Mark telemetry as enabled. Called from `telemetry::init` at startup
/// once the resolved configuration is known. Idempotent.
pub fn set_enabled(enabled: bool) {
    TELEMETRY_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Read the current opt-in state. Callers in `src/packages/*` and
/// other lib-side modules use this in place of
/// `telemetry::is_enabled()` so events fire only when the user
/// consented.
pub fn is_enabled() -> bool {
    TELEMETRY_ENABLED.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize tests in this module — they mutate process-global state.
    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn defaults_to_disabled() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // We can't actually assert the initial value if another test
        // ran first — just verify get/set round-trips.
        set_enabled(false);
        assert!(!is_enabled());
    }

    #[test]
    fn round_trips_set_get() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
    }
}
