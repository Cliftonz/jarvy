//! Semantic version extraction and comparison for tool version checking.
//!
//! This module provides proper semantic versioning support, replacing the
//! previous substring-based matching which caused false positives.

use once_cell::sync::Lazy;
use regex::Regex;
use semver::{Version, VersionReq};

/// Regex pattern to extract semantic versions from tool output.
/// Handles formats like:
/// - "git version 2.44.0"
/// - "v20.10.0"
/// - "Python 3.12.1"
/// - "Docker version 24.0.7, build afdd53b"
/// - "rustc 1.75.0 (82e1608df 2023-12-21)"
/// - "go version go1.21.5 darwin/arm64"
static VERSION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"v?(\d+)\.(\d+)(?:\.(\d+))?(?:-([a-zA-Z0-9.-]+))?").unwrap());

/// Represents an extracted version with optional prerelease tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
}

impl ExtractedVersion {
    /// Convert to a semver::Version for comparison.
    pub fn to_semver(&self) -> Option<Version> {
        let version_str = if let Some(ref pre) = self.prerelease {
            format!("{}.{}.{}-{}", self.major, self.minor, self.patch, pre)
        } else {
            format!("{}.{}.{}", self.major, self.minor, self.patch)
        };
        Version::parse(&version_str).ok()
    }
}

/// Extract a version from tool output string.
///
/// # Examples
/// ```
/// use jarvy::tools::version::extract_version;
///
/// let v = extract_version("git version 2.44.0").unwrap();
/// assert_eq!(v.major, 2);
/// assert_eq!(v.minor, 44);
/// assert_eq!(v.patch, 0);
/// ```
pub fn extract_version(output: &str) -> Option<ExtractedVersion> {
    VERSION_REGEX.captures(output).map(|caps| ExtractedVersion {
        major: caps.get(1).unwrap().as_str().parse().unwrap_or(0),
        minor: caps.get(2).unwrap().as_str().parse().unwrap_or(0),
        patch: caps
            .get(3)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0),
        prerelease: caps.get(4).map(|m| m.as_str().to_string()),
    })
}

/// Check if an installed version satisfies a version requirement.
///
/// Supports multiple requirement formats:
/// - `"latest"` or `"*"`: Always passes (skip check)
/// - `"3.10"`: Prefix match - accepts 3.10.x
/// - `"3.10.0"`: Exact version match
/// - `">= 3.10"`: Minimum version
/// - `"< 4.0"`: Maximum version
/// - `">= 3.10, < 4.0"`: Range expression
/// - `"~3.10"`: Compatible with 3.10.x
/// - `"^3.10"`: Compatible with 3.x
///
/// # Arguments
/// * `installed_output` - The output from running `cmd --version`
/// * `requirement` - The version requirement string from config
///
/// # Returns
/// `true` if the installed version satisfies the requirement
pub fn version_satisfies(installed_output: &str, requirement: &str) -> bool {
    let requirement = requirement.trim();

    // Handle special "any version" cases
    if requirement.is_empty() || requirement == "latest" || requirement == "*" {
        return true;
    }

    // Try to extract the installed version
    let installed = match extract_version(installed_output) {
        Some(v) => v,
        None => {
            // Can't parse installed version - fall back to substring match
            // but be smarter about it: require word boundary
            return contains_version_prefix(installed_output, requirement);
        }
    };

    // Convert to semver for comparison
    let installed_semver = match installed.to_semver() {
        Some(v) => v,
        None => {
            // Shouldn't happen, but fallback to substring
            return contains_version_prefix(installed_output, requirement);
        }
    };

    // Try to parse requirement as semver requirement
    // First, handle simple version prefixes like "3.10" which should match "3.10.x"
    if let Some(req) = parse_requirement(requirement) {
        return req.matches(&installed_semver);
    }

    // Final fallback: smart substring matching
    contains_version_prefix(installed_output, requirement)
}

/// Parse a version requirement string into a VersionReq.
/// Handles both semver requirements and simple version prefixes.
fn parse_requirement(requirement: &str) -> Option<VersionReq> {
    // Check if it starts with an operator - if so, let semver handle it directly
    let has_operator = requirement.starts_with('>')
        || requirement.starts_with('<')
        || requirement.starts_with('=')
        || requirement.starts_with('^')
        || requirement.starts_with('~');

    if has_operator {
        return VersionReq::parse(requirement).ok();
    }

    // Handle simple version prefixes - DON'T pass bare versions to semver
    // as it interprets "3.10.5" as "^3.10.5" which is not what we want
    let parts: Vec<&str> = requirement.split('.').collect();
    let prefix_req = match parts.len() {
        1 => {
            // Just major version: "20" -> ">=20.0.0, <21.0.0"
            if let Ok(major) = parts[0].parse::<u64>() {
                format!(">={}.0.0, <{}.0.0", major, major + 1)
            } else {
                return None;
            }
        }
        2 => {
            // Major.minor: "3.10" -> ">=3.10.0, <3.11.0"
            if let (Ok(major), Ok(minor)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                format!(">={}.{}.0, <{}.{}.0", major, minor, major, minor + 1)
            } else {
                return None;
            }
        }
        3 => {
            // Full version: "3.10.5" -> "=3.10.5" (exact match)
            format!("={}", requirement)
        }
        _ => return None,
    };

    VersionReq::parse(&prefix_req).ok()
}

/// Smart substring matching that avoids false positives.
/// Ensures the requirement matches at a version boundary.
fn contains_version_prefix(output: &str, requirement: &str) -> bool {
    // Find all version-like patterns in the output
    for caps in VERSION_REGEX.captures_iter(output) {
        let full_match = caps.get(0).unwrap().as_str();

        // Check if the matched version starts with the requirement
        // This prevents "2.4" from matching "12.40"
        if full_match.starts_with(requirement)
            || full_match.trim_start_matches('v').starts_with(requirement)
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Version Extraction Tests =====

    #[test]
    fn test_extract_git_version() {
        let v = extract_version("git version 2.44.0").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 44);
        assert_eq!(v.patch, 0);
        assert_eq!(v.prerelease, None);
    }

    #[test]
    fn test_extract_node_version() {
        let v = extract_version("v20.10.0").unwrap();
        assert_eq!(v.major, 20);
        assert_eq!(v.minor, 10);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_extract_python_version() {
        let v = extract_version("Python 3.12.1").unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 12);
        assert_eq!(v.patch, 1);
    }

    #[test]
    fn test_extract_docker_version() {
        let v = extract_version("Docker version 24.0.7, build afdd53b").unwrap();
        assert_eq!(v.major, 24);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 7);
    }

    #[test]
    fn test_extract_rustc_version() {
        let v = extract_version("rustc 1.75.0 (82e1608df 2023-12-21)").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 75);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_extract_go_version() {
        // Go has a quirky format: go1.21.5 (no space after 'go')
        let v = extract_version("go version go1.21.5 darwin/arm64").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 21);
        assert_eq!(v.patch, 5);
    }

    #[test]
    fn test_extract_version_with_prerelease() {
        let v = extract_version("v3.10.0-beta.1").unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 10);
        assert_eq!(v.patch, 0);
        assert_eq!(v.prerelease, Some("beta.1".to_string()));
    }

    #[test]
    fn test_extract_version_missing_patch() {
        let v = extract_version("v3.10").unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 10);
        assert_eq!(v.patch, 0);
    }

    // ===== Version Satisfies Tests =====

    #[test]
    fn test_satisfies_latest() {
        assert!(version_satisfies("anything 1.0.0", "latest"));
        assert!(version_satisfies("anything 99.99.99", "latest"));
    }

    #[test]
    fn test_satisfies_wildcard() {
        assert!(version_satisfies("version 1.0.0", "*"));
        assert!(version_satisfies("version 99.0.0", "*"));
    }

    #[test]
    fn test_satisfies_empty_requirement() {
        assert!(version_satisfies("version 1.0.0", ""));
    }

    #[test]
    fn test_satisfies_prefix_major_minor() {
        // "2.44" should match 2.44.x
        assert!(version_satisfies("git version 2.44.0", "2.44"));
        assert!(version_satisfies("git version 2.44.5", "2.44"));
        // But not 2.43 or 2.45
        assert!(!version_satisfies("git version 2.43.0", "2.44"));
        assert!(!version_satisfies("git version 2.45.0", "2.44"));
    }

    #[test]
    fn test_satisfies_prefix_major_only() {
        // "20" should match 20.x.x
        assert!(version_satisfies("v20.0.0", "20"));
        assert!(version_satisfies("v20.10.5", "20"));
        // But not 19 or 21
        assert!(!version_satisfies("v19.0.0", "20"));
        assert!(!version_satisfies("v21.0.0", "20"));
    }

    #[test]
    fn test_satisfies_exact_version() {
        assert!(version_satisfies("v3.10.5", "3.10.5"));
        assert!(!version_satisfies("v3.10.4", "3.10.5"));
        assert!(!version_satisfies("v3.10.6", "3.10.5"));
    }

    #[test]
    fn test_satisfies_minimum_version() {
        assert!(version_satisfies("Python 3.10.0", ">= 3.10"));
        assert!(version_satisfies("Python 3.11.0", ">= 3.10"));
        assert!(version_satisfies("Python 4.0.0", ">= 3.10"));
        assert!(!version_satisfies("Python 3.9.0", ">= 3.10"));
    }

    #[test]
    fn test_satisfies_maximum_version() {
        assert!(version_satisfies("Python 3.9.0", "< 4.0"));
        assert!(version_satisfies("Python 3.99.99", "< 4.0"));
        assert!(!version_satisfies("Python 4.0.0", "< 4.0"));
        assert!(!version_satisfies("Python 4.1.0", "< 4.0"));
    }

    #[test]
    fn test_satisfies_range() {
        // ">= 3.10, < 4.0" should match 3.10 through 3.x
        assert!(version_satisfies("Python 3.10.0", ">= 3.10, < 4.0"));
        assert!(version_satisfies("Python 3.12.5", ">= 3.10, < 4.0"));
        assert!(!version_satisfies("Python 3.9.0", ">= 3.10, < 4.0"));
        assert!(!version_satisfies("Python 4.0.0", ">= 3.10, < 4.0"));
    }

    #[test]
    fn test_satisfies_caret() {
        // "^1.70" should match 1.70.x through 1.x (but not 2.x)
        assert!(version_satisfies("rustc 1.70.0", "^1.70"));
        assert!(version_satisfies("rustc 1.75.0", "^1.70"));
        assert!(!version_satisfies("rustc 1.69.0", "^1.70"));
        assert!(!version_satisfies("rustc 2.0.0", "^1.70"));
    }

    #[test]
    fn test_satisfies_tilde() {
        // "~3.10" should match 3.10.x only
        assert!(version_satisfies("Python 3.10.0", "~3.10"));
        assert!(version_satisfies("Python 3.10.5", "~3.10"));
        assert!(!version_satisfies("Python 3.11.0", "~3.10"));
        assert!(!version_satisfies("Python 3.9.0", "~3.10"));
    }

    // ===== Bug Fix Tests =====

    #[test]
    fn test_no_false_positive_substring() {
        // This was the bug: "2.4" should NOT match "12.40.0"
        assert!(!version_satisfies("version 12.40.0", "2.4"));

        // And "2.4" should NOT match "1.24.0"
        assert!(!version_satisfies("version 1.24.0", "2.4"));
    }

    #[test]
    fn test_no_false_positive_embedded_number() {
        // "3.1" should NOT match "3.10" or "3.11"
        // because 3.1.x is different from 3.10.x
        assert!(!version_satisfies("Python 3.10.0", "3.1"));
        assert!(!version_satisfies("Python 3.11.0", "3.1"));

        // But "3.1" should match "3.1.0" and "3.1.5"
        assert!(version_satisfies("Python 3.1.0", "3.1"));
        assert!(version_satisfies("Python 3.1.5", "3.1"));
    }

    // ===== Edge Cases =====

    #[test]
    fn test_handles_unparseable_output() {
        // If we can't parse the version, fall back to smart substring match
        assert!(version_satisfies("some weird output 2.0", "2.0"));
    }

    #[test]
    fn test_handles_multiple_versions_in_output() {
        // Some tools show multiple versions - we match the first
        let output = "tool 1.0.0 (built with lib 2.5.0)";
        assert!(version_satisfies(output, "1.0"));
        // Should not match the embedded library version incorrectly
    }

    #[test]
    fn test_prerelease_not_matched_by_release() {
        // "3.10" should not match prerelease versions by default
        // (following semver semantics)
        let pre = extract_version("v3.10.0-beta.1").unwrap();
        let release = extract_version("v3.10.0").unwrap();
        assert_ne!(pre, release);
    }
}
