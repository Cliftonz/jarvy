//! Framework auto-detection from project filesystem layout.

use super::config::HookFramework;
use std::path::Path;

/// Detect a hook framework by probing well-known config paths. Returns
/// `None` if nothing matches — caller treats that as "no hook
/// integration for this project."
///
/// Order: husky (`.husky/` or `"husky"` in `package.json`) → lefthook
/// (`lefthook.yml`/`.yaml`). Native has no auto-detection signal — a
/// team using the native handler always declares it explicitly via
/// `[git_hooks.native]` in `jarvy.toml`.
pub fn detect_framework(project_dir: &Path) -> Option<HookFramework> {
    if project_dir.join(".husky").is_dir() {
        return Some(HookFramework::Husky);
    }

    // Husky-via-package.json: scan for the husky devDependency or
    // prepare script. String-match is fine here — we don't need to
    // parse JSON to make a coarse detection decision.
    let package_json = project_dir.join("package.json");
    if package_json.exists()
        && let Ok(content) = std::fs::read_to_string(&package_json)
        && content.contains("\"husky\"")
    {
        return Some(HookFramework::Husky);
    }

    if project_dir.join("lefthook.yml").exists() || project_dir.join("lefthook.yaml").exists() {
        return Some(HookFramework::Lefthook);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn empty_dir_returns_none() {
        let tmp = tempdir().unwrap();
        assert_eq!(detect_framework(tmp.path()), None);
    }

    #[test]
    fn husky_directory_detected() {
        let tmp = tempdir().unwrap();
        fs::create_dir(tmp.path().join(".husky")).unwrap();
        assert_eq!(detect_framework(tmp.path()), Some(HookFramework::Husky));
    }

    #[test]
    fn husky_via_package_json_detected() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{ "devDependencies": { "husky": "^9.0.0" } }"#,
        )
        .unwrap();
        assert_eq!(detect_framework(tmp.path()), Some(HookFramework::Husky));
    }

    #[test]
    fn lefthook_yml_detected() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("lefthook.yml"), "pre-commit: {}").unwrap();
        assert_eq!(detect_framework(tmp.path()), Some(HookFramework::Lefthook));
    }

    #[test]
    fn pre_commit_config_no_longer_detected_as_framework() {
        // Pre-commit framework support was removed in v0.8. A project
        // with a `.pre-commit-config.yaml` sitting in the tree should
        // NOT trigger jarvy to try to install pre-commit — the user
        // opts into native hooks via `[git_hooks.native]` explicitly.
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".pre-commit-config.yaml"), "repos: []").unwrap();
        assert_eq!(detect_framework(tmp.path()), None);
    }
}
