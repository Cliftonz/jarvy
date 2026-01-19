//! CI command handlers - generate CI configs and show CI info

use crate::ci;
use crate::error_codes;

/// Run the ci-config command
pub fn run_ci_config(provider: ci::CiProvider, output: &str, dry_run: bool) {
    let template = match ci::CiConfigTemplate::for_provider(provider) {
        Some(t) => t,
        None => {
            eprintln!(
                "Error: CI config generation is not supported for {}",
                provider
            );
            eprintln!("Supported providers: github, gitlab, circleci, azure, bitbucket");
            std::process::exit(error_codes::CONFIG_ERROR);
        }
    };

    if dry_run {
        println!("=== {} ===", template.file_path);
        println!("{}", template.content);
    } else {
        let base_path = std::path::Path::new(output);
        match template.write(base_path) {
            Ok(path) => {
                println!("Generated CI config: {}", path.display());
                println!("Provider: {}", template.provider);
                println!("Description: {}", template.description);
            }
            Err(e) => {
                eprintln!("Error generating CI config: {}", e);
                std::process::exit(error_codes::CONFIG_ERROR);
            }
        }
    }
}

/// Run the ci-info command
pub fn run_ci_info() {
    match ci::detect() {
        Some(env) => {
            println!("CI Environment Detected");
            println!("=======================");
            println!("Provider: {}", env.provider);
            println!("Forced: {}", env.forced);
            println!();
            println!("Features:");
            println!("  - Log groups: {}", env.provider.supports_groups());
            println!("  - Output vars: {}", env.provider.supports_output_vars());
            println!("  - Caching: {}", env.provider.supports_cache());
            if let Some(cache_dir) = env.provider.cache_dir() {
                println!("  - Cache dir: {}", cache_dir);
            }
            println!();
            println!("Build Information:");
            if let Some(ref id) = env.build_id {
                println!("  - Build ID: {}", id);
            }
            if let Some(ref repo) = env.repository {
                println!("  - Repository: {}", repo);
            }
            if let Some(ref branch) = env.branch {
                println!("  - Branch: {}", branch);
            }
            if let Some(ref sha) = env.commit_sha {
                println!("  - Commit: {}", sha);
            }
        }
        None => {
            println!("Not running in a CI environment.");
            println!();
            println!("Supported CI providers:");
            println!("  - GitHub Actions (GITHUB_ACTIONS=true)");
            println!("  - GitLab CI (GITLAB_CI=true)");
            println!("  - CircleCI (CIRCLECI=true)");
            println!("  - Travis CI (TRAVIS=true)");
            println!("  - Azure DevOps (TF_BUILD=True)");
            println!("  - Jenkins (JENKINS_URL set)");
            println!("  - Bitbucket (BITBUCKET_BUILD_NUMBER set)");
            println!("  - Buildkite (BUILDKITE=true)");
            println!("  - TeamCity (TEAMCITY_VERSION set)");
            println!("  - AppVeyor (APPVEYOR=True)");
            println!("  - Generic (CI=true)");
            println!();
            println!("Use --ci flag to force CI mode, or set JARVY_CI=1");
        }
    }
}
