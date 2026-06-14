//! Jarvy-managed entry markers, consolidated.
//!
//! Each on-disk format uses a slightly different shape to tag the entries
//! we own — JSON uses a sentinel key, YAML uses comment fences, Cline
//! uses a filename infix. Keeping the strings together in one module
//! prevents drift (e.g. accidentally using `_` in one and `-` in
//! another) and gives auditors a single source of truth.

/// JSON key used on every Jarvy-managed entry inside an agent settings
/// file. Paired with the entry's content hash to detect impersonation
/// before `remove` strips it.
pub const JSON_MARKER_KEY: &str = "_jarvy_managed";

/// JSON key holding the content hash of a Jarvy-managed entry. Computed
/// over (name, event, matcher, command, windows_command, timeout_ms).
/// Used by `remove` / `check` to refuse foreign entries that impersonate
/// the marker.
pub const JSON_HASH_KEY: &str = "_jarvy_sha256";

/// Begin sentinel for the Jarvy-managed block in Continue's YAML.
pub const YAML_BLOCK_BEGIN: &str = "# jarvy-managed begin";

/// End sentinel for the Jarvy-managed block in Continue's YAML.
pub const YAML_BLOCK_END: &str = "# jarvy-managed end";

/// Infix in Cline fragment filenames: `<Event>.jarvy.<hook-name>.sh`.
pub const FILENAME_INFIX: &str = ".jarvy.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_marker_key_uses_underscore_convention() {
        // YAML uses kebab (`jarvy-managed`), JSON uses snake
        // (`_jarvy_managed`). Pin both so the next reader sees the
        // convention divergence is intentional.
        assert_eq!(JSON_MARKER_KEY, "_jarvy_managed");
        assert_eq!(JSON_HASH_KEY, "_jarvy_sha256");
        assert!(YAML_BLOCK_BEGIN.contains("jarvy-managed"));
        assert!(YAML_BLOCK_END.contains("jarvy-managed"));
        assert_eq!(FILENAME_INFIX, ".jarvy.");
    }
}
