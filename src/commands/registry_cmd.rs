//! `jarvy registry {sync,status,clear}` command handler.
//!
//! Thin shell over `crate::registry_remote::sync`. The handler returns an
//! exit code so `main.rs` can short-circuit on error without unwinding
//! through a panic.

use crate::cli::RegistryAction;
use crate::error_codes;
use crate::registry_remote;

pub fn run_registry(action: &RegistryAction) -> i32 {
    match action {
        RegistryAction::Sync {} => run_sync(),
        RegistryAction::Status {} => run_status(),
        RegistryAction::Clear {} => run_clear(),
    }
}

fn run_sync() -> i32 {
    println!("Syncing remote registry...");
    match registry_remote::run_sync() {
        Ok(report) => {
            println!();
            println!("[ok] Registry sync complete");
            println!("  Source:           {}", report.registry_url);
            println!("  Tools synced:     {}", report.tools_synced);
            println!("  Tools removed:    {}", report.tools_removed);
            println!("  Duration:         {}ms", report.duration_ms);
            println!(
                "  Signature:        {}",
                if report.signature_verified {
                    "verified (cosign)"
                } else {
                    "NOT verified (require_signature=false)"
                }
            );
            println!();
            println!("Run `jarvy setup` or `jarvy validate` — synced tools are now available.");
            0
        }
        Err(e) => {
            eprintln!("Registry sync failed: {}", e);
            crate::observability::telemetry_gate::emit(|| {
                tracing::error!(
                    event = "registry.cli.sync_failed",
                    error_kind = error_kind(&e),
                    error = %e,
                );
            });
            // Map the most common failures to specific exit codes; fall
            // back to CONFIG_ERROR for everything else (matches the
            // "config-shaped failure" semantics callers already handle).
            match e {
                registry_remote::SyncError::NotConfigured => error_codes::CONFIG_ERROR,
                registry_remote::SyncError::Fetch(_) => error_codes::NETWORK_TIMEOUT,
                _ => error_codes::CONFIG_ERROR,
            }
        }
    }
}

/// Classify a `SyncError` for the `error_kind` tracing field. Keeps the
/// label set bounded so OTLP labels don't cardinality-bomb.
fn error_kind(e: &registry_remote::SyncError) -> &'static str {
    use registry_remote::SyncError as E;
    match e {
        E::NotConfigured => "not_configured",
        E::UnsafeConfig(_) => "unsafe_config",
        E::Fetch(_) => "fetch",
        E::Manifest(_) => "manifest",
        E::Cache(_) => "cache",
        E::Signature(_) => "signature",
        E::CosignBackend(_) => "cosign",
        E::ShaMismatch { .. } => "sha_mismatch",
        E::ToolParseFailed { .. } => "tool_parse_failed",
    }
}

fn run_status() -> i32 {
    let meta_path = match crate::paths::registry_remote_cache_dir() {
        Ok(d) => d.join("meta.json"),
        Err(e) => {
            eprintln!("Cannot resolve registry cache dir: {}", e);
            return error_codes::CONFIG_ERROR;
        }
    };

    if !meta_path.exists() {
        println!("No registry sync recorded yet. Run `jarvy registry sync` to fetch.");
        return 0;
    }

    match std::fs::read_to_string(&meta_path) {
        Ok(content) => {
            println!("{}", content);
            0
        }
        Err(e) => {
            eprintln!("Cannot read {}: {}", meta_path.display(), e);
            error_codes::CONFIG_ERROR
        }
    }
}

fn run_clear() -> i32 {
    let cache_dir = match crate::paths::registry_remote_cache_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Cannot resolve registry cache dir: {}", e);
            return error_codes::CONFIG_ERROR;
        }
    };

    if !cache_dir.exists() {
        println!("Registry cache already empty.");
        return 0;
    }

    match std::fs::remove_dir_all(&cache_dir) {
        Ok(_) => {
            println!("Cleared registry cache at {}.", cache_dir.display());
            0
        }
        Err(e) => {
            eprintln!("Cannot clear {}: {}", cache_dir.display(), e);
            error_codes::CONFIG_ERROR
        }
    }
}
