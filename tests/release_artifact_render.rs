//! Regression test for release artifact placeholder rendering.
//!
//! Reason: v0.0.5 shipped because v0.0.3 left literal `VERSION_PLACEHOLDER`
//! tokens in `chocolateyinstall.ps1` (the workflow only substituted the
//! nuspec). The fix lives in workflow YAML; a typo in `sed`, a renamed token,
//! or a third file with a placeholder reproduces the same Chocolatey
//! moderation rejection days later.
//!
//! This test simulates the release-time substitution against every templated
//! file under `dist/` and asserts no `_PLACEHOLDER` token survives. It also
//! validates the resulting nuspec parses as XML.

use std::fs;
use std::path::PathBuf;

const FAKE_VERSION: &str = "0.0.0-test";
const FAKE_SHA256: &str = "DEADBEEFCAFE0000DEADBEEFCAFE0000DEADBEEFCAFE0000DEADBEEFCAFE0000";

fn dist_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist")
}

/// Apply the same substitutions the release workflows perform:
/// any `*_PLACEHOLDER*` token is replaced with the corresponding fake value.
/// Order matters — replace the longer (suffixed) SHA256 forms before the
/// bare `SHA256_PLACEHOLDER`, otherwise the bare form swallows the suffix.
fn render(content: &str) -> String {
    let mut out = content.to_string();
    // Suffixed SHA256 variants first.
    for token in [
        "SHA256_PLACEHOLDER_LINUX_ARM",
        "SHA256_PLACEHOLDER_LINUX_X86",
        "SHA256_PLACEHOLDER_MACOS_ARM",
        "SHA256_PLACEHOLDER_MACOS_X86",
        "SHA256_PLACEHOLDER_ARM",
        "SHA256_PLACEHOLDER_X86",
    ] {
        out = out.replace(token, FAKE_SHA256);
    }
    out = out.replace("SHA256_PLACEHOLDER", FAKE_SHA256);
    out = out.replace("VERSION_PLACEHOLDER", FAKE_VERSION);
    out
}

fn templated_files() -> Vec<PathBuf> {
    let dist = dist_root();
    [
        "windows/chocolatey/jarvy.nuspec",
        "windows/chocolatey/tools/chocolateyinstall.ps1",
        "windows/winget.yaml",
        "homebrew/jarvy.rb",
        "debian/control",
        "rpm/jarvy.spec",
        "aur/PKGBUILD",
        "aur/PKGBUILD-bin",
    ]
    .iter()
    .map(|rel| dist.join(rel))
    .collect()
}

#[test]
fn every_templated_file_exists() {
    for path in templated_files() {
        assert!(
            path.exists(),
            "expected templated release artifact at {}",
            path.display()
        );
    }
}

#[test]
fn rendered_artifacts_have_no_placeholder_tokens() {
    let mut failures: Vec<String> = Vec::new();
    for path in templated_files() {
        let raw =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
        let rendered = render(&raw);
        if rendered.contains("_PLACEHOLDER") {
            // Pull a few sample lines so the failure shows what wasn't substituted.
            let samples: Vec<&str> = rendered
                .lines()
                .filter(|l| l.contains("_PLACEHOLDER"))
                .take(5)
                .collect();
            failures.push(format!(
                "{} retains `_PLACEHOLDER` after substitution:\n  {}",
                path.display(),
                samples.join("\n  ")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "release artifacts retained placeholders:\n{}",
        failures.join("\n")
    );
}

#[test]
fn nuspec_renders_as_well_formed_xml() {
    let path = dist_root().join("windows/chocolatey/jarvy.nuspec");
    let raw = fs::read_to_string(&path).unwrap();
    let rendered = render(&raw);
    // Cheap structural check — every <openTag> has a matching </closeTag>
    // counted by occurrence, no need for a full XML parser dep.
    assert_balanced_tag(&rendered, "package");
    assert_balanced_tag(&rendered, "metadata");
    assert_balanced_tag(&rendered, "files");
    assert!(
        rendered.contains(&format!("<version>{FAKE_VERSION}</version>")),
        "rendered nuspec missing substituted version"
    );
}

fn assert_balanced_tag(xml: &str, tag: &str) {
    // Anchor the open match on a non-name character (`<package ` or `<package>`)
    // so `<packageSourceUrl>` doesn't get counted as an open of `<package>`.
    let opens = xml.matches(&format!("<{tag} ")).count() + xml.matches(&format!("<{tag}>")).count();
    let closes = xml.matches(&format!("</{tag}>")).count();
    assert_eq!(
        opens, closes,
        "nuspec tag <{tag}> unbalanced: {opens} opens vs {closes} closes"
    );
}

#[test]
fn chocolateyinstall_ps1_braces_are_balanced() {
    let path = dist_root().join("windows/chocolatey/tools/chocolateyinstall.ps1");
    let raw = fs::read_to_string(&path).unwrap();
    let rendered = render(&raw);
    let opens = rendered.matches('{').count();
    let closes = rendered.matches('}').count();
    assert_eq!(
        opens, closes,
        "chocolateyinstall.ps1 braces unbalanced: {opens} opens vs {closes} closes"
    );
}

#[test]
fn sample_substitutions_match_workflow_behavior() {
    // Sentinel cases pulled from the actual templates so a future rename of a
    // placeholder token surfaces here, not in Chocolatey moderation.
    let cases: &[(&str, &str)] = &[
        (
            "InstallerUrl: https://github.com/bearbinary/jarvy/releases/download/vVERSION_PLACEHOLDER/x.zip",
            "InstallerUrl: https://github.com/bearbinary/jarvy/releases/download/v0.0.0-test/x.zip",
        ),
        (
            "$checksum64  = 'SHA256_PLACEHOLDER'",
            &format!("$checksum64  = '{FAKE_SHA256}'"),
        ),
        (
            "sha256sums_aarch64=('SHA256_PLACEHOLDER_ARM')",
            &format!("sha256sums_aarch64=('{FAKE_SHA256}')"),
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(render(input), *expected, "render({input:?})");
    }
}
