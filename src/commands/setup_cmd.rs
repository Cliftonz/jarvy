//! Setup command handler - the main environment setup command
//!
//! This module contains the setup command implementation which:
//! - Installs tools from jarvy.toml
//! - Executes hooks (pre_setup, post_install, post_setup)
//! - Configures environment variables
//! - Auto-starts services

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use crate::ci;
use crate::config::Config;
use crate::env::{
    DotenvConfig, EnvContext, SecretsConfig, ShellConfig, collect_secrets, detect_shell,
    generate_dotenv, preview_dotenv, preview_shell_rc, update_shell_rc,
};
use crate::error_codes;
use crate::hooks::{Hook, HookConfig, HookEnv};
use crate::onboarding::mark_initialized;
use crate::packages;
use crate::remote::fetch_remote_config;
use crate::services;
use crate::setup::setup;
use crate::telemetry;
use crate::tools;

/// Run the setup command
#[allow(unsafe_code)] // SAFETY: env vars set at startup before spawning threads
#[allow(clippy::too_many_arguments)]
pub fn run_setup(
    file: &str,
    from: Option<&str>,
    role: Option<&str>,
    no_hooks: bool,
    dry_run: bool,
    ci_flag: bool,
    no_ci: bool,
    jobs: usize,
    sequential: bool,
    ignore_missing_deps: bool,
    insecure: bool,
    header: &[String],
) {
    // Determine effective parallelism level
    let parallel_jobs = if sequential { 1 } else { jobs.max(1) };

    // Set env var for dependency warning suppression
    if ignore_missing_deps {
        // SAFETY: Setting env var at startup before spawning threads
        unsafe { std::env::set_var("JARVY_IGNORE_MISSING_DEPS", "1") };
    }

    // Handle CI mode detection with CLI overrides
    // SAFETY: We're setting env vars at startup before any threads are spawned
    let ci_env = if ci_flag {
        // Force CI mode
        unsafe { std::env::set_var("JARVY_CI", "1") };
        ci::detect()
    } else if no_ci {
        // Force non-CI mode
        unsafe { std::env::set_var("JARVY_NO_CI", "1") };
        None
    } else {
        ci::detect()
    };

    // Log CI detection
    if let Some(ref env) = ci_env {
        let output = env.output();
        output.notice(&format!("Running in CI mode: {}", env.provider));
        if let Some(ref build_id) = env.build_id {
            output.debug(&format!("Build ID: {}", build_id));
        }
    }

    // Determine config file path: fetch from URL or use local file
    let config_path = if let Some(url) = from {
        match fetch_remote_config(url, insecure, header) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error fetching remote config: {}", e);
                std::process::exit(error_codes::CONFIG_ERROR);
            }
        }
    } else {
        file.to_string()
    };

    let config = Config::new(&config_path);
    let hooks_config = config.get_hooks();
    let hook_settings = HookConfig::from(&hooks_config.config);

    // Set the global default for sudo usage based on config
    tools::set_default_use_sudo(config.use_sudo());

    // Execute pre_setup hook if configured
    if !no_hooks {
        if let Some(ref script) = hooks_config.pre_setup {
            let hook = Hook::with_config(script, "pre_setup", hook_settings.clone())
                .with_env(HookEnv::global());
            if dry_run {
                hook.dry_run();
            } else {
                match hook.execute() {
                    Ok(_) => {}
                    Err(e) => {
                        if !hook_settings.continue_on_error {
                            eprintln!("Pre-setup hook failed: {}", e);
                            std::process::exit(error_codes::HOOK_FAILED);
                        }
                        eprintln!("Warning: Pre-setup hook failed: {}", e);
                    }
                }
            }
        }
    }

    if !dry_run {
        setup();
    } else {
        println!("[DRY-RUN] Would run platform setup");
    }

    // Get tool configs with role override if --role flag was used
    if let Some(role_name) = role {
        println!("Using role override: {}", role_name);
    }
    let tool_configs = config.get_tool_configs_with_role_override(role);

    // Phase 2: Parallel version checking - determine which tools need installation
    println!("Checking tool versions...");
    let version_check = tools::spec::check_tools_parallel(
        tool_configs
            .values()
            .map(|t| (t.name.as_str(), t.version.as_str())),
    );

    // Report version check results
    println!("{}", version_check.summary_string());

    // Log already-satisfied tools (verbose mode)
    if !version_check.satisfied.is_empty() {
        println!(
            "Already installed: {}",
            version_check
                .satisfied
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Log unknown tools - critical for MCP feedback loop
    for (name, version) in &version_check.unknown {
        let msg = format!(
            "We do not currently have support for {} package but we have logged it and will be adding it soon.",
            name
        );
        eprintln!("{}", msg);
        // Emit telemetry for unknown tool (used by MCP feedback)
        telemetry::tool_not_supported(name, Some(version), telemetry::Source::Config);
        if !telemetry::is_enabled() {
            eprintln!(
                "Telemetry is disabled. Please consider creating a feature request here: https://github.com/bearbinary/Jarvy/issues/new"
            );
        }
    }

    // Create list of known tools for hook execution (needed later)
    let known_tools: Vec<_> = tool_configs
        .iter()
        .filter(|(_, t)| tools::get_tool(&t.name).is_some())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // Only install tools that actually need installation (from parallel check)
    // First, order tools by dependencies to ensure version managers are installed first
    let ordered_tools = tools::spec::order_tools_by_dependencies(
        version_check
            .needs_install
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str())),
    );

    // Group tools by package manager for batch installation
    let tool_groups = tools::spec::group_tools_for_installation(
        ordered_tools.iter().map(|(n, v)| (n.as_str(), v.as_str())),
    );

    // Track successfully installed tools for hook execution
    let mut successfully_installed: Vec<(String, String)> = Vec::new();

    if dry_run {
        // Dry-run: show what would be installed
        for (pm, packages) in &tool_groups.by_package_manager {
            let package_names: Vec<&str> =
                packages.iter().map(|(_, pkg, _)| pkg.as_str()).collect();
            println!(
                "[DRY-RUN] Would batch install via {:?}: {}",
                pm,
                package_names.join(", ")
            );
        }
        for (name, version) in &tool_groups.custom_install {
            println!(
                "[DRY-RUN] Would install {} version {} using custom installer",
                name, version
            );
        }
    } else {
        // Emit setup_started event
        telemetry::setup_started(version_check.needs_install.len());
        let _setup_start = telemetry::now();

        // Batch install by package manager
        for (pm, packages) in &tool_groups.by_package_manager {
            if packages.is_empty() {
                continue;
            }

            // Emit tool_requested for each tool in the batch
            for (tool_name, _, version) in packages {
                telemetry::tool_requested(tool_name, version, telemetry::Source::Config);
            }

            let package_names: Vec<&str> =
                packages.iter().map(|(_, pkg, _)| pkg.as_str()).collect();
            println!(
                "Batch installing {} packages via {:?}: {}",
                packages.len(),
                pm,
                package_names.join(", ")
            );

            let install_start = telemetry::now();
            match tools::common::PkgOps::batch_install(*pm, &package_names, None) {
                Ok(result) => {
                    let batch_duration = install_start.elapsed();
                    // Track successful installs
                    for pkg_name in &result.succeeded {
                        // Find the tool name for this package
                        if let Some((tool_name, _, version)) =
                            packages.iter().find(|(_, pkg, _)| pkg == pkg_name)
                        {
                            println!("Successfully installed {} ({})", tool_name, version);
                            successfully_installed.push((tool_name.clone(), version.clone()));
                            telemetry::tool_installed(
                                tool_name,
                                version,
                                &format!("{:?}", pm),
                                batch_duration,
                            );
                        }
                    }
                    // Log failures
                    for (pkg_name, error) in &result.failed {
                        if let Some((tool_name, _, version)) =
                            packages.iter().find(|(_, pkg, _)| pkg == pkg_name)
                        {
                            let msg =
                                format!("Failed to install {} ({}): {}", tool_name, version, error);
                            eprintln!("{}", msg);
                            telemetry::tool_failed(tool_name, version, error);
                        }
                    }
                }
                Err(e) => {
                    // Batch install failed entirely - log all as failed
                    for (tool_name, _, version) in packages {
                        let msg = format!("Failed to install {} ({}): {:?}", tool_name, version, e);
                        eprintln!("{}", msg);
                        telemetry::tool_failed(tool_name, version, &format!("{:?}", e));
                    }
                }
            }
        }

        // Install custom tools with configurable parallelism
        if !tool_groups.custom_install.is_empty() {
            // Emit tool_requested for each custom tool
            for (name, version) in &tool_groups.custom_install {
                telemetry::tool_requested(name, version, telemetry::Source::Config);
            }

            let custom_count = tool_groups.custom_install.len();
            let effective_jobs = parallel_jobs.min(custom_count);

            if effective_jobs > 1 {
                println!(
                    "Installing {} custom tools with {} parallel jobs",
                    custom_count, effective_jobs
                );

                // Configure thread pool for this installation phase
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(effective_jobs)
                    .build()
                    .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

                // Thread-safe collectors for results
                let success_collector: Arc<Mutex<Vec<(String, String)>>> =
                    Arc::new(Mutex::new(Vec::new()));
                let error_collector: Arc<Mutex<Vec<(String, String, String)>>> =
                    Arc::new(Mutex::new(Vec::new()));

                pool.install(|| {
                    tool_groups
                        .custom_install
                        .par_iter()
                        .for_each(|(name, version)| {
                            println!(
                                "Installing {} version {} using custom installer",
                                name, version
                            );

                            match tools::add(name, version) {
                                Ok(()) => {
                                    println!("Successfully installed {} ({})", name, version);
                                    if let Ok(mut guard) = success_collector.lock() {
                                        guard.push((name.clone(), version.clone()));
                                    }
                                }
                                Err(e) => {
                                    let msg = format!(
                                        "Failed to install {} ({}): {:?}",
                                        name, version, e
                                    );
                                    eprintln!("{}", msg);
                                    if let Ok(mut guard) = error_collector.lock() {
                                        guard.push((
                                            name.clone(),
                                            version.clone(),
                                            format!("{:?}", e),
                                        ));
                                    }
                                }
                            }
                        });
                });

                // Merge successful installs
                if let Ok(guard) = success_collector.lock() {
                    successfully_installed.extend(guard.iter().cloned());
                }

                // Report errors to telemetry
                if let Ok(guard) = error_collector.lock() {
                    for (name, version, error) in guard.iter() {
                        telemetry::tool_failed(name, version, error);
                    }
                }
            } else {
                // Sequential installation (--sequential or --jobs 1)
                for (name, version) in &tool_groups.custom_install {
                    println!(
                        "Installing {} version {} using custom installer",
                        name, version
                    );

                    match tools::add(name, version) {
                        Ok(()) => {
                            println!("Successfully installed {} ({})", name, version);
                            successfully_installed.push((name.clone(), version.clone()));
                        }
                        Err(e) => {
                            let msg = format!("Failed to install {} ({}): {:?}", name, version, e);
                            eprintln!("{}", msg);
                            telemetry::tool_failed(name, version, &format!("{:?}", e));
                        }
                    }
                }
            }
        }

        // Execute hooks for successfully installed tools
        if !no_hooks {
            for (tool_name, version) in &successfully_installed {
                let user_hook = config
                    .get_tool_hooks(tool_name)
                    .and_then(|h| h.post_install.as_ref());

                if let Some(script) = user_hook {
                    // User-provided hook takes precedence
                    let env = HookEnv::for_tool(tool_name, version);
                    let hook = Hook::with_config(
                        script,
                        &format!("{} post_install", tool_name),
                        hook_settings.clone(),
                    )
                    .with_env(env);
                    match hook.execute() {
                        Ok(_) => {}
                        Err(e) => {
                            if !hook_settings.continue_on_error {
                                eprintln!("Post-install hook for {} failed: {}", tool_name, e);
                                std::process::exit(error_codes::HOOK_FAILED);
                            }
                            eprintln!("Warning: Post-install hook for {} failed: {}", tool_name, e);
                        }
                    }
                } else if let Some(default_hook) = tools::spec::get_tool_default_hook(tool_name) {
                    // Fall back to tool's built-in default hook
                    println!(
                        "Running default hook for {}: {}",
                        tool_name, default_hook.description
                    );
                    let env = HookEnv::for_tool(tool_name, version);
                    let hook = Hook::with_config(
                        default_hook.script,
                        &format!("{} default_hook", tool_name),
                        hook_settings.clone(),
                    )
                    .with_env(env);
                    match hook.execute() {
                        Ok(_) => {}
                        Err(e) => {
                            // Default hooks are advisory; always continue on error
                            eprintln!("Warning: Default hook for {} failed: {}", tool_name, e);
                        }
                    }
                }
            }
        }
    }

    // Show dry-run for per-tool hooks
    if dry_run && !no_hooks {
        for (_, tool) in &known_tools {
            let user_hook = config
                .get_tool_hooks(&tool.name)
                .and_then(|h| h.post_install.as_ref());

            if let Some(script) = user_hook {
                let env = HookEnv::for_tool(&tool.name, &tool.version);
                let hook = Hook::with_config(
                    script,
                    &format!("{} post_install", tool.name),
                    hook_settings.clone(),
                )
                .with_env(env);
                hook.dry_run();
            } else if let Some(default_hook) = tools::spec::get_tool_default_hook(&tool.name) {
                // Show default hook in dry-run
                println!(
                    "[DRY-RUN] Would run default hook for {}: {}",
                    tool.name, default_hook.description
                );
                let env = HookEnv::for_tool(&tool.name, &tool.version);
                let hook = Hook::with_config(
                    default_hook.script,
                    &format!("{} default_hook", tool.name),
                    hook_settings.clone(),
                )
                .with_env(env);
                hook.dry_run();
            }
        }
    }

    // Install language-specific packages (npm, pip, cargo)
    if config.has_packages() {
        let packages_config = config.get_packages_config();
        let project_dir = std::path::Path::new(file)
            .parent()
            .unwrap_or(std::path::Path::new("."));

        if dry_run {
            println!("\n=== Package Dependencies (dry-run) ===");
            if packages_config.npm.is_some() {
                println!("[DRY-RUN] Would install npm packages");
            }
            if packages_config.pip.is_some() {
                println!("[DRY-RUN] Would install pip packages");
            }
            if packages_config.cargo.is_some() {
                println!("[DRY-RUN] Would install cargo binaries");
            }
        } else {
            println!("\n=== Installing Package Dependencies ===");
            if let Err(e) = packages::install_packages(&packages_config, project_dir) {
                eprintln!("Warning: Package installation failed: {}", e);
            }
        }
    }

    // Git configuration
    if config.has_git() {
        if let Some(git_config) = config.get_git() {
            if dry_run {
                println!("\n=== Git Configuration (dry-run) ===");
                if let Some(ref name) = git_config.user_name {
                    if let Some(resolved) = name.resolve() {
                        println!("[DRY-RUN] Would set git config user.name: {}", resolved);
                    }
                }
                if let Some(ref email) = git_config.user_email {
                    if let Some(resolved) = email.resolve() {
                        println!("[DRY-RUN] Would set git config user.email: {}", resolved);
                    }
                }
                if git_config.signing {
                    println!("[DRY-RUN] Would enable commit signing");
                    if let Some(ref key) = git_config.signing_key {
                        println!("[DRY-RUN] Would set signing key: {}", key);
                    }
                }
                if let Some(ref branch) = git_config.default_branch {
                    println!("[DRY-RUN] Would set init.defaultBranch: {}", branch);
                }
                if !git_config.aliases.is_empty() {
                    println!(
                        "[DRY-RUN] Would configure {} git aliases",
                        git_config.aliases.len()
                    );
                }
            } else {
                println!("\n=== Git Configuration ===");
                let setup = crate::git::GitSetup::new(git_config.clone());
                match setup.configure() {
                    Ok(()) => println!("Git configuration applied successfully"),
                    Err(e) => eprintln!("Warning: Git configuration failed: {}", e),
                }
            }
        }
    }

    // Environment variable setup
    let env_config = config.get_env();
    let env_settings = &env_config.config;

    if !env_config.vars.is_empty() || !env_config.secrets.is_empty() {
        // Build environment context
        let ctx = EnvContext::new();

        // Collect secrets if any (skip in CI mode or if dry run)
        let secrets_config = SecretsConfig {
            ci_mode: std::env::var("CI").is_ok()
                || std::env::var("JARVY_CI").is_ok()
                || std::env::var("JARVY_TEST_MODE").is_ok()
                || dry_run,
            fail_on_missing: false,
        };

        let secrets = if !dry_run && !env_config.secrets.is_empty() {
            match collect_secrets(&env_config.secrets, &ctx, &secrets_config) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Warning: Could not collect secrets: {}", e);
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        // Merge vars and secrets
        let mut all_vars: HashMap<String, String> = env_config
            .vars
            .iter()
            .map(|(k, v)| (k.clone(), v.value().to_string()))
            .collect();
        all_vars.extend(secrets);

        // Generate .env file if configured
        if env_settings.generate_dotenv {
            let dotenv_path = std::path::Path::new(".env");
            let dotenv_config = DotenvConfig {
                backup: true,
                force: false,
                add_to_gitignore: env_settings.add_to_gitignore,
            };

            if dry_run {
                println!("\n=== Environment Setup (dry-run) ===");
                println!(
                    "[DRY-RUN] Would generate .env file at {}",
                    dotenv_path.display()
                );
                let preview = preview_dotenv(&all_vars, &ctx);
                println!("{}", preview);
            } else {
                match generate_dotenv(dotenv_path, &all_vars, &ctx, &dotenv_config) {
                    Ok(_) => println!("\nGenerated .env file at {}", dotenv_path.display()),
                    Err(e) => eprintln!("Warning: Could not generate .env file: {}", e),
                }
            }
        }

        // Update shell rc file if configured
        if env_settings.update_rc {
            let shell = detect_shell();
            let shell_config = ShellConfig {
                backup: true,
                validate: false,
            };

            if dry_run {
                if !env_settings.generate_dotenv {
                    println!("\n=== Environment Setup (dry-run) ===");
                }
                println!("[DRY-RUN] Would update shell rc for {}", shell);
                let preview = preview_shell_rc(shell, &all_vars, &ctx);
                println!("{}", preview);
            } else {
                match update_shell_rc(shell, &all_vars, &ctx, &shell_config) {
                    Ok(path) => println!("Updated shell rc at {}", path.display()),
                    Err(e) => eprintln!("Warning: Could not update shell rc: {}", e),
                }
            }
        }
    }

    // Execute post_setup hook if configured
    if !no_hooks {
        if let Some(ref script) = hooks_config.post_setup {
            let hook = Hook::with_config(script, "post_setup", hook_settings.clone())
                .with_env(HookEnv::global());
            if dry_run {
                hook.dry_run();
            } else {
                match hook.execute() {
                    Ok(_) => {}
                    Err(e) => {
                        if !hook_settings.continue_on_error {
                            eprintln!("Post-setup hook failed: {}", e);
                            std::process::exit(error_codes::HOOK_FAILED);
                        }
                        eprintln!("Warning: Post-setup hook failed: {}", e);
                    }
                }
            }
        }
    }

    // Auto-start services if configured
    let services_config = &config.services;
    let is_ci = ci_env.is_some();
    if services_config.should_auto_start(is_ci) {
        let working_dir = std::path::Path::new(file)
            .parent()
            .unwrap_or(std::path::Path::new("."));

        // Detect service backend
        let backend_result = services::detect_backend_with_config(
            working_dir,
            services_config.compose_file.as_deref(),
            services_config.tilt_file.as_deref(),
        );

        if let Some((backend, config_path)) = backend_result {
            let backend_impl = services::get_backend(backend);

            if backend_impl.is_installed() {
                if dry_run {
                    println!("\n[DRY-RUN] Would auto-start {} services", backend);
                } else {
                    println!("\nAuto-starting {} services...", backend);
                    match backend_impl.start(&config_path, true) {
                        Ok(result) => {
                            println!("{}", result.message);
                        }
                        Err(e) => {
                            // Services auto-start is advisory - don't fail the setup
                            eprintln!("Warning: Failed to auto-start services: {}", e);
                        }
                    }
                }
            } else {
                eprintln!(
                    "Note: {} config found but {} is not installed. Skipping services auto-start.",
                    backend, backend
                );
            }
        }
    }

    if config.has_hooks() && !no_hooks {
        println!("\nHooks execution summary:");
        if hooks_config.pre_setup.is_some() {
            println!("  - pre_setup: executed");
        }
        let tool_hooks_count = hooks_config
            .tool_hooks
            .values()
            .filter(|h| h.post_install.is_some())
            .count();
        if tool_hooks_count > 0 {
            println!("  - tool post_install hooks: {} executed", tool_hooks_count);
        }
        if hooks_config.post_setup.is_some() {
            println!("  - post_setup: executed");
        }
    }

    // Capture environment state for drift detection
    if !dry_run {
        let drift_config = config.drift.clone().unwrap_or_default();
        if drift_config.enabled {
            let project_dir = std::path::Path::new(file)
                .parent()
                .unwrap_or(std::path::Path::new("."));

            let mut state = crate::drift::EnvironmentState::new();

            // Capture tool states
            for (tool_name, tool) in &known_tools {
                if let Ok(path) = which::which(tool_name) {
                    state.set_tool(
                        tool_name,
                        &tool.version,
                        &path,
                        &detect_install_method(tool_name),
                    );
                }
            }

            // Capture tracked file hashes
            for file_path in &drift_config.track_files {
                let full_path = project_dir.join(file_path);
                if full_path.exists() {
                    if let Ok(hash) = crate::drift::state::hash_file(&full_path) {
                        state.set_file_hash(file_path, &hash);
                    }
                }
            }

            // Capture config file hash
            let config_path = project_dir.join("jarvy.toml");
            if config_path.exists() {
                if let Ok(hash) = crate::drift::state::hash_file(&config_path) {
                    state.set_config_hash(&hash);
                }
            }

            // Save state
            if let Err(e) = state.save(project_dir) {
                eprintln!("Warning: Could not save drift detection state: {}", e);
            } else {
                println!(
                    "\nDrift detection baseline captured ({} tools)",
                    state.tool_count()
                );
            }
        }
    }

    // Mark as initialized after successful setup (first-run complete)
    if !dry_run {
        let _ = mark_initialized();
    }
}

/// Detect install method for a tool based on its path
fn detect_install_method(tool: &str) -> String {
    if let Ok(path) = which::which(tool) {
        let path_str = path.to_string_lossy();

        if path_str.contains("/homebrew/") || path_str.contains("/opt/homebrew/") {
            return "brew".to_string();
        }
        if path_str.contains("/.cargo/") {
            return "cargo".to_string();
        }
        if path_str.contains("/.nvm/") {
            return "nvm".to_string();
        }
        if path_str.contains("/.pyenv/") {
            return "pyenv".to_string();
        }
        if path_str.contains("/.rustup/") {
            return "rustup".to_string();
        }
        if path_str.contains("/usr/bin/") || path_str.contains("/usr/local/bin/") {
            return "system".to_string();
        }
    }

    "unknown".to_string()
}
