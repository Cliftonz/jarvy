//! Shared JSON-merge helpers for the four agents that store hooks in a
//! JSON array (Claude Code, Cursor, Codex, Windsurf).
//!
//! Each helper enforces the same marker-based merge rule: Jarvy owns
//! entries whose `_jarvy_managed` matches AND whose `_jarvy_sha256`
//! recomputes to the current LIBRARY signature. Foreign entries that
//! impersonate the marker but fail the hash check are **left in place**
//! by `strip_all_jarvy` — refusal to delete protects user-authored hooks
//! from a hostile / typo'd settings file.

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::ResolvedEntry;
use super::markers::{JSON_HASH_KEY, JSON_MARKER_KEY};

/// Hash the load-bearing fields of a `ResolvedEntry`. Stable across runs
/// for the same library hook version. Hex-encoded SHA-256 (64 chars).
pub fn entry_hash(entry: &ResolvedEntry) -> String {
    let mut h = Sha256::new();
    h.update(entry.name.as_bytes());
    h.update([0]);
    h.update(entry.event.as_str().as_bytes());
    h.update([0]);
    h.update(entry.matcher.as_deref().unwrap_or("").as_bytes());
    h.update([0]);
    h.update(entry.bash_command.as_bytes());
    h.update([0]);
    h.update(entry.windows_command.as_bytes());
    h.update([0]);
    h.update(entry.timeout_ms.to_le_bytes());
    hex::encode(h.finalize())
}

/// Drop any existing Jarvy-managed entry with the same name (regardless
/// of hash) so callers can re-insert the current version. Returns the
/// count removed.
pub fn retain_non_jarvy_named(arr: &mut Vec<Value>, name: &str) -> usize {
    let before = arr.len();
    arr.retain(|v| {
        v.get(JSON_MARKER_KEY)
            .and_then(|s| s.as_str())
            .map(|s| s != name)
            .unwrap_or(true)
    });
    before - arr.len()
}

/// Strip every Jarvy-managed entry from `arr` and return
/// `(stripped, foreign_preserved)`.
///
/// An entry is considered Jarvy-owned if it has the marker key AND one of:
///
/// * No `_jarvy_sha256` field at all — treated as a legacy entry from
///   before the impersonation defense shipped. Stripped silently.
/// * A `_jarvy_sha256` field whose value matches a known
///   `(name, hash)` pair from the library / current desired state.
///
/// Entries with the marker present but a `_jarvy_sha256` that does NOT
/// match any known pair are **preserved**. The marker is plain JSON, so
/// anyone with write access to the settings file can impersonate it; we
/// refuse to delete on impersonation, surface the count as
/// `foreign_preserved`, and let the operator investigate.
///
/// Currently unused by `remove` (which sweeps every marker entry —
/// foreign-impersonation defense lives in `apply`). Kept available for
/// a future hash-aware removal mode (e.g. `jarvy ai-hooks remove
/// --refuse-foreign`).
#[allow(dead_code)]
pub fn strip_jarvy_entries(arr: &mut Vec<Value>, known: &[(&str, &str)]) -> (usize, usize) {
    let mut foreign = 0usize;
    let before = arr.len();
    arr.retain(|v| {
        let Some(marker) = v.get(JSON_MARKER_KEY).and_then(|s| s.as_str()) else {
            return true; // not a jarvy entry — keep
        };
        let hash_field = v.get(JSON_HASH_KEY).and_then(|s| s.as_str());
        match hash_field {
            None => false, // legacy entry — strip
            Some(hash) => {
                if known
                    .iter()
                    .any(|(name, expected_hash)| *name == marker && *expected_hash == hash)
                {
                    false // jarvy-owned + hash matches → strip
                } else {
                    foreign += 1;
                    true // marker present but hash mismatched → foreign, preserve
                }
            }
        }
    });
    (before - arr.len(), foreign)
}

/// Collect the names of all Jarvy-managed entries currently on disk. Used
/// by `check` to compute drift against the desired set. Ignores hash
/// mismatches — a foreign entry impersonating the marker shows up as
/// `extra_jarvy` so the operator can investigate.
pub fn collect_marker_names(arr: &[Value]) -> Vec<String> {
    arr.iter()
        .filter_map(|v| {
            v.get(JSON_MARKER_KEY)
                .and_then(|s| s.as_str())
                .map(String::from)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_hooks::event::HookEvent;
    use serde_json::json;
    use std::borrow::Cow;

    fn sample_entry(name: &str) -> ResolvedEntry<'_> {
        ResolvedEntry {
            name: name.to_string(),
            library_source: Some(name.to_string()),
            event: HookEvent::PreToolUse,
            matcher: Some("Bash".to_string()),
            bash_command: Cow::Borrowed("exit 0\n"),
            windows_command: Cow::Borrowed("exit 0\n"),
            windows_warned: false,
            timeout_ms: 5_000,
        }
    }

    #[test]
    fn entry_hash_is_stable() {
        let a = entry_hash(&sample_entry("block-rm-rf"));
        let b = entry_hash(&sample_entry("block-rm-rf"));
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn entry_hash_changes_with_command_body() {
        let a = entry_hash(&sample_entry("block-rm-rf"));
        let mut b = sample_entry("block-rm-rf");
        b.bash_command = Cow::Borrowed("exit 1\n");
        assert_ne!(a, entry_hash(&b));
    }

    #[test]
    fn retain_drops_matching_name() {
        let mut arr = vec![
            json!({ JSON_MARKER_KEY: "block-rm-rf" }),
            json!({ "matcher": "Edit" }),
            json!({ JSON_MARKER_KEY: "block-force-push" }),
        ];
        let removed = retain_non_jarvy_named(&mut arr, "block-rm-rf");
        assert_eq!(removed, 1);
        assert_eq!(arr.len(), 2);
        assert!(arr.iter().all(|v| {
            v.get(JSON_MARKER_KEY)
                .and_then(|s| s.as_str())
                .map(|s| s != "block-rm-rf")
                .unwrap_or(true)
        }));
    }

    #[test]
    fn strip_keeps_foreign_marker_impersonation() {
        let mut arr = vec![
            json!({ JSON_MARKER_KEY: "block-rm-rf", JSON_HASH_KEY: "abc" }),
            json!({ JSON_MARKER_KEY: "block-rm-rf", JSON_HASH_KEY: "DIFFERENT" }),
            json!({ "user_hook": true }),
        ];
        let (stripped, foreign) = strip_jarvy_entries(&mut arr, &[("block-rm-rf", "abc")]);
        assert_eq!(stripped, 1);
        assert_eq!(foreign, 1);
        // User entry + foreign impersonation remain.
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn strip_handles_legacy_entries_with_no_hash() {
        // Entries written before the impersonation defense shipped have
        // the marker key but no _jarvy_sha256 — strip them on sight.
        let mut arr = vec![
            json!({ JSON_MARKER_KEY: "block-rm-rf" }),
            json!({ "user_hook": true }),
        ];
        let (stripped, foreign) = strip_jarvy_entries(&mut arr, &[]);
        assert_eq!(stripped, 1);
        assert_eq!(foreign, 0);
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn collect_marker_names_handles_mixed_entries() {
        let arr = vec![
            json!({ JSON_MARKER_KEY: "a" }),
            json!({ "user": true }),
            json!({ JSON_MARKER_KEY: "b" }),
        ];
        let names = collect_marker_names(&arr);
        assert_eq!(names, vec!["a", "b"]);
    }
}
