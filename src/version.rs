//! Tag-aware version constant, shared by the lib and bin crate roots.

/// The version jarvy reports (`--version`) and compares against during
/// self-update. Release builds bake the exact git tag in at compile time
/// (release.yml exports `JARVY_BUILD_TAG="${tag#v}"` before `cargo
/// build`), so an rc binary reports `0.7.0-rc.1` instead of the bare
/// Cargo version — rc iterations were previously indistinguishable by
/// `--version` (#54), and the update checker compared `0.X.Y` as newer
/// than every `0.X.Y-rc.*`, so rc users were never offered the next rc.
/// Dev / `cargo install` builds fall back to the crate version.
pub const JARVY_VERSION: &str = match option_env!("JARVY_BUILD_TAG") {
    Some(tag) => tag,
    None => env!("CARGO_PKG_VERSION"),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_nonempty_semverish() {
        assert!(!JARVY_VERSION.is_empty());
        // Never carries a leading 'v' — release.yml strips it before
        // exporting JARVY_BUILD_TAG, and Cargo versions never have one.
        assert!(!JARVY_VERSION.starts_with('v'));
        assert!(JARVY_VERSION.split('.').count() >= 3);
    }
}
