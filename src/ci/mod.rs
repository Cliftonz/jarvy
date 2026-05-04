//! CI/CD Detection and Integration module
//!
//! This module provides functionality for:
//! - Auto-detecting CI/CD environments (GitHub Actions, GitLab CI, CircleCI, etc.)
//! - Provider-specific output formatting (log groups, warnings, errors)
//! - CI config generation for major providers
//! - Non-interactive mode handling

mod config;
mod output;

// Public API exports - these may not be used internally but are part of the module's interface
#[allow(unused_imports)]
pub use config::{CiConfigError, CiConfigTemplate, generate_ci_config};
#[allow(unused_imports)]
pub use output::{CiOutput, GroupGuard};

use crate::telemetry;
use std::env;

/// Supported CI/CD providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiProvider {
    /// GitHub Actions (GITHUB_ACTIONS=true)
    GitHubActions,
    /// GitLab CI (GITLAB_CI=true)
    GitLabCi,
    /// CircleCI (CIRCLECI=true)
    CircleCi,
    /// Travis CI (TRAVIS=true)
    TravisCi,
    /// Azure DevOps Pipelines (TF_BUILD=True)
    AzureDevOps,
    /// Jenkins (JENKINS_URL set)
    Jenkins,
    /// Bitbucket Pipelines (BITBUCKET_BUILD_NUMBER set)
    Bitbucket,
    /// Buildkite (BUILDKITE=true)
    Buildkite,
    /// TeamCity (TEAMCITY_VERSION set)
    TeamCity,
    /// AppVeyor (APPVEYOR=True)
    AppVeyor,
    /// Generic CI (CI=true but no specific provider detected)
    Generic,
}

impl CiProvider {
    /// Returns the human-readable name of the CI provider
    pub fn name(&self) -> &'static str {
        match self {
            Self::GitHubActions => "GitHub Actions",
            Self::GitLabCi => "GitLab CI",
            Self::CircleCi => "CircleCI",
            Self::TravisCi => "Travis CI",
            Self::AzureDevOps => "Azure DevOps",
            Self::Jenkins => "Jenkins",
            Self::Bitbucket => "Bitbucket Pipelines",
            Self::Buildkite => "Buildkite",
            Self::TeamCity => "TeamCity",
            Self::AppVeyor => "AppVeyor",
            Self::Generic => "Generic CI",
        }
    }

    /// Returns true if this provider supports log grouping
    pub fn supports_groups(&self) -> bool {
        matches!(
            self,
            Self::GitHubActions | Self::GitLabCi | Self::AzureDevOps | Self::Buildkite
        )
    }

    /// Returns true if this provider supports setting output variables
    pub fn supports_output_vars(&self) -> bool {
        matches!(self, Self::GitHubActions | Self::AzureDevOps)
    }

    /// Returns true if this provider supports caching
    pub fn supports_cache(&self) -> bool {
        matches!(
            self,
            Self::GitHubActions
                | Self::GitLabCi
                | Self::CircleCi
                | Self::Bitbucket
                | Self::Buildkite
        )
    }

    /// Returns the cache directory path for this provider, if known
    pub fn cache_dir(&self) -> Option<&'static str> {
        match self {
            Self::GitHubActions => Some("/home/runner/.cache"),
            Self::GitLabCi => Some("/cache"),
            Self::CircleCi => Some("/home/circleci/.cache"),
            _ => None,
        }
    }
}

impl std::fmt::Display for CiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// CI detection result containing provider information
#[derive(Debug, Clone)]
pub struct CiEnvironment {
    /// The detected CI provider
    pub provider: CiProvider,
    /// Whether CI mode is forced via CLI flag (--ci)
    pub forced: bool,
    /// The build/job ID if available
    pub build_id: Option<String>,
    /// The repository information if available
    pub repository: Option<String>,
    /// The branch name if available
    pub branch: Option<String>,
    /// The commit SHA if available
    pub commit_sha: Option<String>,
}

impl CiEnvironment {
    /// Creates a new CI environment with the detected provider
    pub fn new(provider: CiProvider) -> Self {
        let (build_id, repository, branch, commit_sha) = match provider {
            CiProvider::GitHubActions => (
                env::var("GITHUB_RUN_ID").ok(),
                env::var("GITHUB_REPOSITORY").ok(),
                env::var("GITHUB_REF_NAME").ok(),
                env::var("GITHUB_SHA").ok(),
            ),
            CiProvider::GitLabCi => (
                env::var("CI_JOB_ID").ok(),
                env::var("CI_PROJECT_PATH").ok(),
                env::var("CI_COMMIT_BRANCH").ok(),
                env::var("CI_COMMIT_SHA").ok(),
            ),
            CiProvider::CircleCi => (
                env::var("CIRCLE_BUILD_NUM").ok(),
                env::var("CIRCLE_PROJECT_REPONAME").ok(),
                env::var("CIRCLE_BRANCH").ok(),
                env::var("CIRCLE_SHA1").ok(),
            ),
            CiProvider::TravisCi => (
                env::var("TRAVIS_BUILD_ID").ok(),
                env::var("TRAVIS_REPO_SLUG").ok(),
                env::var("TRAVIS_BRANCH").ok(),
                env::var("TRAVIS_COMMIT").ok(),
            ),
            CiProvider::AzureDevOps => (
                env::var("BUILD_BUILDID").ok(),
                env::var("BUILD_REPOSITORY_NAME").ok(),
                env::var("BUILD_SOURCEBRANCHNAME").ok(),
                env::var("BUILD_SOURCEVERSION").ok(),
            ),
            CiProvider::Jenkins => (
                env::var("BUILD_ID").ok(),
                env::var("GIT_URL").ok(),
                env::var("GIT_BRANCH").ok(),
                env::var("GIT_COMMIT").ok(),
            ),
            CiProvider::Bitbucket => (
                env::var("BITBUCKET_BUILD_NUMBER").ok(),
                env::var("BITBUCKET_REPO_FULL_NAME").ok(),
                env::var("BITBUCKET_BRANCH").ok(),
                env::var("BITBUCKET_COMMIT").ok(),
            ),
            CiProvider::Buildkite => (
                env::var("BUILDKITE_BUILD_ID").ok(),
                env::var("BUILDKITE_REPO").ok(),
                env::var("BUILDKITE_BRANCH").ok(),
                env::var("BUILDKITE_COMMIT").ok(),
            ),
            CiProvider::TeamCity => (
                env::var("BUILD_NUMBER").ok(),
                None,
                env::var("BRANCH_NAME").ok(),
                env::var("BUILD_VCS_NUMBER").ok(),
            ),
            CiProvider::AppVeyor => (
                env::var("APPVEYOR_BUILD_ID").ok(),
                env::var("APPVEYOR_REPO_NAME").ok(),
                env::var("APPVEYOR_REPO_BRANCH").ok(),
                env::var("APPVEYOR_REPO_COMMIT").ok(),
            ),
            CiProvider::Generic => (None, None, None, None),
        };

        Self {
            provider,
            forced: false,
            build_id,
            repository,
            branch,
            commit_sha,
        }
    }

    /// Creates a forced CI environment (for --ci flag)
    pub fn forced(provider: CiProvider) -> Self {
        let mut env = Self::new(provider);
        env.forced = true;
        env
    }

    /// Returns a CiOutput helper for this environment
    pub fn output(&self) -> CiOutput {
        CiOutput::new(self.provider)
    }
}

/// Detects the current CI environment by checking environment variables.
///
/// Returns `Some(CiEnvironment)` if running in a CI environment,
/// `None` if running locally (interactive mode).
///
/// Detection order:
/// 1. Check JARVY_NO_CI=1 to force non-CI mode
/// 2. Check JARVY_CI=1 to force CI mode (generic)
/// 3. Check specific provider variables (in order of popularity)
/// 4. Check generic CI=true as fallback
pub fn detect() -> Option<CiEnvironment> {
    // Allow forcing non-CI mode
    if env::var("JARVY_NO_CI").as_deref() == Ok("1") {
        return None;
    }

    // Allow forcing CI mode via env var
    if env::var("JARVY_CI").as_deref() == Ok("1") {
        return Some(CiEnvironment::forced(
            detect_provider().unwrap_or(CiProvider::Generic),
        ));
    }

    let ci_env = detect_provider().map(CiEnvironment::new);

    // Emit telemetry if CI detected
    if let Some(ref env) = ci_env {
        telemetry::ci_detected(
            env.provider.name(),
            env.build_id.as_deref(),
            env.branch.as_deref(),
        );
    }

    ci_env
}

/// Detects the specific CI provider without checking force flags
fn detect_provider() -> Option<CiProvider> {
    // Check specific providers first (in order of popularity)

    // GitHub Actions
    if env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        return Some(CiProvider::GitHubActions);
    }

    // GitLab CI
    if env::var("GITLAB_CI").as_deref() == Ok("true") {
        return Some(CiProvider::GitLabCi);
    }

    // CircleCI
    if env::var("CIRCLECI").as_deref() == Ok("true") {
        return Some(CiProvider::CircleCi);
    }

    // Travis CI
    if env::var("TRAVIS").as_deref() == Ok("true") {
        return Some(CiProvider::TravisCi);
    }

    // Azure DevOps (TF_BUILD can be "True" with capital T)
    if env::var("TF_BUILD")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Some(CiProvider::AzureDevOps);
    }

    // Buildkite
    if env::var("BUILDKITE").as_deref() == Ok("true") {
        return Some(CiProvider::Buildkite);
    }

    // AppVeyor (APPVEYOR can be "True" with capital T)
    if env::var("APPVEYOR")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Some(CiProvider::AppVeyor);
    }

    // Jenkins (check if JENKINS_URL is set)
    if env::var("JENKINS_URL").is_ok() {
        return Some(CiProvider::Jenkins);
    }

    // Bitbucket Pipelines (check if BITBUCKET_BUILD_NUMBER is set)
    if env::var("BITBUCKET_BUILD_NUMBER").is_ok() {
        return Some(CiProvider::Bitbucket);
    }

    // TeamCity (check if TEAMCITY_VERSION is set)
    if env::var("TEAMCITY_VERSION").is_ok() {
        return Some(CiProvider::TeamCity);
    }

    // Generic CI detection (fallback)
    if env::var("CI").as_deref() == Ok("true") || env::var("CI").as_deref() == Ok("1") {
        return Some(CiProvider::Generic);
    }

    None
}

/// Returns true if running in a CI environment
#[allow(dead_code)] // Public API for library consumers
pub fn is_ci() -> bool {
    detect().is_some()
}

/// Returns true if running in a CI environment or test mode
#[allow(dead_code)] // Public API for library consumers
pub fn is_non_interactive() -> bool {
    is_ci() || env::var("JARVY_TEST_MODE").as_deref() == Ok("1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Use a mutex to serialize tests that modify environment variables
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Every CI provider env var jarvy looks at. `with_env` clears all of
    /// these before setting the test's target vars so the test runs with
    /// a known-empty CI baseline. Without this, tests run in a real CI
    /// (GitHub Actions sets GITHUB_ACTIONS=true, CI=true, RUNNER_OS=...)
    /// see the runner's vars leaking into detect() and consistently fail
    /// because detect() returns the runner's provider instead of the
    /// test's target. Keep this list in sync with the detect() function
    /// in this module — anything detect() reads MUST be cleared here.
    const CI_PROVIDER_VARS: &[&str] = &[
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "CIRCLECI",
        "TRAVIS",
        "TF_BUILD",
        "JENKINS_URL",
        "BITBUCKET_BUILD_NUMBER",
        "BUILDKITE",
        "TEAMCITY_VERSION",
        "APPVEYOR",
        "JARVY_NO_CI",
        "JARVY_CI",
    ];

    #[allow(unsafe_code)]
    fn with_env<F, R>(vars: &[(&str, &str)], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        // Recover from poisoned mutex: a previous test's panic shouldn't
        // cascade-fail every other test that needs the env-isolation lock.
        // The data inside is only ever a unit guard, so taking the
        // poisoned guard back is safe.
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // SAFETY: Tests run single-threaded with ENV_LOCK mutex.
        //
        // Step 1: snapshot every known CI provider var and clear them all.
        // This isolates the test from the runner's CI vars (GITHUB_ACTIONS,
        // CI, etc.) which would otherwise make detect() return the wrong
        // provider regardless of what the test sets.
        let cleared: Vec<(&str, Option<String>)> = CI_PROVIDER_VARS
            .iter()
            .map(|k| {
                let orig = env::var(k).ok();
                unsafe { env::remove_var(k) };
                (*k, orig)
            })
            .collect();

        // Step 2: snapshot and set the test's target vars on top of the
        // cleared baseline. Tracked separately from `cleared` because
        // these may overlap and we want set-then-restore semantics.
        let originals: Vec<_> = vars
            .iter()
            .map(|(k, v)| {
                let orig = env::var(k).ok();
                unsafe { env::set_var(k, v) };
                (*k, orig)
            })
            .collect();

        let result = f();

        // Restore the test-target vars first, then the cleared baseline.
        // Order matters: a test target var could also be in the cleared
        // list, in which case the cleared restore should be the final
        // word.
        for (k, orig) in originals {
            match orig {
                Some(v) => unsafe { env::set_var(k, v) },
                None => unsafe { env::remove_var(k) },
            }
        }
        for (k, orig) in cleared {
            match orig {
                Some(v) => unsafe { env::set_var(k, v) },
                None => unsafe { env::remove_var(k) },
            }
        }

        result
    }

    #[allow(unsafe_code)]
    fn with_cleared_env<F, R>(vars_to_clear: &[&str], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        // Recover from poisoned mutex: a previous test's panic shouldn't
        // cascade-fail every other test that needs the env-isolation lock.
        // The data inside is only ever a unit guard, so taking the
        // poisoned guard back is safe.
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Save original values and clear them
        // SAFETY: Tests run single-threaded with ENV_LOCK mutex
        let originals: Vec<_> = vars_to_clear
            .iter()
            .map(|k| {
                let orig = env::var(k).ok();
                unsafe { env::remove_var(k) };
                (*k, orig)
            })
            .collect();

        let result = f();

        // Restore original values
        for (k, orig) in originals {
            if let Some(v) = orig {
                unsafe { env::set_var(k, v) };
            }
        }

        result
    }

    #[test]
    fn test_github_actions_detection() {
        with_env(&[("GITHUB_ACTIONS", "true")], || {
            let ci = detect();
            assert!(ci.is_some());
            let env = ci.unwrap();
            assert_eq!(env.provider, CiProvider::GitHubActions);
            assert!(env.provider.supports_groups());
            assert!(env.provider.supports_output_vars());
        });
    }

    #[test]
    fn test_gitlab_ci_detection() {
        with_env(&[("GITLAB_CI", "true")], || {
            let ci = detect();
            assert!(ci.is_some());
            let env = ci.unwrap();
            assert_eq!(env.provider, CiProvider::GitLabCi);
            assert!(env.provider.supports_groups());
        });
    }

    #[test]
    fn test_circleci_detection() {
        with_env(&[("CIRCLECI", "true")], || {
            let ci = detect();
            assert!(ci.is_some());
            assert_eq!(ci.unwrap().provider, CiProvider::CircleCi);
        });
    }

    #[test]
    fn test_azure_devops_detection() {
        // Azure DevOps uses "True" with capital T
        with_env(&[("TF_BUILD", "True")], || {
            let ci = detect();
            assert!(ci.is_some());
            let env = ci.unwrap();
            assert_eq!(env.provider, CiProvider::AzureDevOps);
            assert!(env.provider.supports_groups());
            assert!(env.provider.supports_output_vars());
        });
    }

    #[test]
    fn test_jenkins_detection() {
        with_env(&[("JENKINS_URL", "http://jenkins.example.com")], || {
            let ci = detect();
            assert!(ci.is_some());
            assert_eq!(ci.unwrap().provider, CiProvider::Jenkins);
        });
    }

    #[test]
    fn test_generic_ci_detection() {
        with_env(&[("CI", "true")], || {
            let ci = detect();
            assert!(ci.is_some());
            assert_eq!(ci.unwrap().provider, CiProvider::Generic);
        });
    }

    #[test]
    fn test_jarvy_ci_force() {
        with_env(&[("JARVY_CI", "1")], || {
            let ci = detect();
            assert!(ci.is_some());
            let env = ci.unwrap();
            assert!(env.forced);
        });
    }

    #[test]
    fn test_jarvy_no_ci_override() {
        with_env(&[("CI", "true"), ("JARVY_NO_CI", "1")], || {
            let ci = detect();
            assert!(ci.is_none());
        });
    }

    #[test]
    fn test_provider_name() {
        assert_eq!(CiProvider::GitHubActions.name(), "GitHub Actions");
        assert_eq!(CiProvider::GitLabCi.name(), "GitLab CI");
        assert_eq!(CiProvider::Generic.name(), "Generic CI");
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(format!("{}", CiProvider::GitHubActions), "GitHub Actions");
    }

    #[test]
    fn test_no_ci_detected() {
        with_cleared_env(
            &[
                "CI",
                "GITHUB_ACTIONS",
                "GITLAB_CI",
                "CIRCLECI",
                "TRAVIS",
                "TF_BUILD",
                "JENKINS_URL",
                "BITBUCKET_BUILD_NUMBER",
                "BUILDKITE",
                "TEAMCITY_VERSION",
                "APPVEYOR",
                "JARVY_CI",
            ],
            || {
                let ci = detect_provider();
                assert!(ci.is_none());
            },
        );
    }
}
