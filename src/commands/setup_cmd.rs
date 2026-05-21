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
    header: &[String],
    machine_id: Option<&str>,
) -> i32 {
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
        match fetch_remote_config(url, header) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error fetching remote config: {}", e);
                return error_codes::CONFIG_ERROR;
            }
        }
    } else {
        file.to_string()
    };

    // Generate a per-invocation correlation ID. Every event emitted from
    // this `run_setup` call carries `run_id` so support can stitch parallel
    // install threads back together when reading `~/.jarvy/logs/jarvy.log`
    // or an OTLP backend (observability review #6, #7).
    let run_id = uuid::Uuid::now_v7();
    // Hold this span open for the entire setup so `tracing::info_span!`
    // child spans inherit `run_id` automatically.
    let setup_span = tracing::info_span!("setup", run_id = %run_id);
    let _setup_span_guard = setup_span.enter();

    // Startup banner — the first five questions support asks ("what
    // version, what OS, what package manager, what config source, what CI
    // provider"). Without this the only signal in a ticket bundle is
    // `eprintln!`s scattered through the install loop (observability
    // review #11).
    tracing::info!(
        event = "setup.start",
        run_id = %run_id,
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        config_source = %if from.is_some() { "remote" } else { "local" },
        config_path = %config_path,
        ci_provider = ?ci_env.as_ref().map(|e| e.provider.to_string()),
        jobs = parallel_jobs,
        dry_run = dry_run,
    );

    let config = Config::new_with_workspace(&config_path);
    let hooks_config = config.get_hooks();
    let hook_settings = HookConfig::from(&hooks_config.config);

    // Set the global default for sudo usage based on config
    tools::set_default_use_sudo(config.use_sudo());

    // Execute pre_setup hook if configured. Routed through the shared
    // Hook::run_with_policy helper so the four-line "fail-or-continue"
    // refrain isn't repeated 4× in this function (maintainability
    // review #11).
    if !no_hooks {
        if let Some(ref script) = hooks_config.pre_setup {
            let hook = Hook::with_config(script, "pre_setup", hook_settings.clone())
                .with_env(HookEnv::global());
            if hook.run_with_policy(dry_run).is_err() {
                return error_codes::HOOK_FAILED;
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

    // Emit full tool inventory for security audit via OTEL
    telemetry::setup_inventory(
        &tool_configs
            .values()
            .map(|t| (t.name.clone(), t.version.clone()))
            .collect::<Vec<_>>(),
        role,
        file,
        machine_id,
    );

    // Phase 2: Parallel version checking - determine which tools need installation
    println!("Checking tool versions...");
    let version_check = tools::spec::check_tools_parallel(
        tool_configs
            .values()
            .map(|t| (t.name.as_str(), t.version.as_str())),
    );

    // Report version check results
    println!("{}", version_check.summary_string());

    // Verify-only fallback (PRD-053). If we're in a sandbox that
    // can't install (read-only rootfs, sudoless + no user-scope
    // package manager), don't even try — report gaps and exit. The
    // doctor pipeline ran inline as `version_check` above, so we
    // already know what's missing.
    //
    // Auto-baseline runs *inside* this branch on the success path
    // too: a pre-loaded sandbox image that already satisfies the
    // config should leave behind a drift baseline regardless of
    // whether installs were possible, because subsequent sessions
    // need that baseline to do meaningful drift checks.
    if !dry_run
        && let Some(env) = crate::sandbox::detect()
        && let crate::sandbox::InstallCapability::VerifyOnly(reason) =
            crate::sandbox::install_capability()
    {
        if version_check.needs_install.is_empty() && version_check.unknown.is_empty() {
            let project_dir = std::path::Path::new(file)
                .parent()
                .unwrap_or(std::path::Path::new("."));
            if !crate::paths::state_json(project_dir).exists() {
                let known_tools_for_baseline: Vec<(&String, &crate::config::Tool)> = tool_configs
                    .iter()
                    .filter(|(_, t)| tools::get_tool(&t.name).is_some())
                    .collect();
                capture_drift_baseline_borrowed(
                    project_dir,
                    std::path::Path::new(file),
                    &known_tools_for_baseline,
                    &[],
                    /* auto = */ true,
                );
            }
            tracing::info!(
                event = "setup.verify_only.passed",
                provider = %env.provider,
                reason = %reason,
                "sandbox verify-only mode passed; all configured tools present"
            );
            eprintln!(
                "[jarvy] sandbox cannot install tools (read-only or no package manager); \
                 all configured tools already present — verify-only mode passed"
            );
            return error_codes::EXIT_SUCCESS;
        } else {
            // Build the missing list once into a single buffer; avoids
            // intermediate Vec<&str> + String allocation pair.
            let mut missing = String::with_capacity(64);
            for (i, (n, _)) in version_check.needs_install.iter().enumerate() {
                if i > 0 {
                    missing.push_str(", ");
                }
                missing.push_str(n);
            }
            tracing::warn!(
                event = "setup.verify_only.refused",
                provider = %env.provider,
                reason = %reason,
                missing = %missing,
                "sandbox cannot install tools"
            );
            eprintln!(
                "[jarvy] sandbox cannot install tools (read-only or no package manager); \
                 missing: {missing}"
            );
            return error_codes::PREREQ_MISSING;
        }
    }

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

    // Log unknown tools - critical for MCP feedback loop. Suppress
    // the telemetry-disabled nag in seamless mode (PRD-053) — those
    // environments are usually multi-tenant or ephemeral, and the
    // nag is noise the operator can't act on.
    let seamless = crate::sandbox::is_seamless();
    for (name, version) in &version_check.unknown {
        let msg = format!(
            "We do not currently have support for {} package but we have logged it and will be adding it soon.",
            name
        );
        eprintln!("{}", msg);
        // Emit telemetry for unknown tool (used by MCP feedback)
        telemetry::tool_not_supported(name, Some(version), telemetry::Source::Config);
        if !telemetry::is_enabled() && !seamless {
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

                // Capture the parent span (which carries `run_id`) so rayon
                // worker threads — which have their own thread-local span
                // stack — can re-enter it. Without this, every event emitted
                // from a parallel-install worker is span-orphaned in
                // `~/.jarvy/logs/jarvy.log` and OTLP traces (observability
                // review #7).
                let parent_span = tracing::Span::current();
                pool.install(|| {
                    tool_groups
                        .custom_install
                        .par_iter()
                        .for_each(|(name, version)| {
                            let _g = parent_span.enter();
                            let install_span = tracing::info_span!(
                                "install_tool",
                                tool = %name,
                                version = %version,
                            );
                            let _is = install_span.enter();
                            let install_start = std::time::Instant::now();
                            println!(
                                "Installing {} version {} using custom installer",
                                name, version
                            );

                            match tools::add(name, version) {
                                Ok(()) => {
                                    println!("Successfully installed {} ({})", name, version);
                                    // Round-2 obs F13: emit tool.installed
                                    // for the custom-install path. The
                                    // batch-install path already does
                                    // this; without the matching call here
                                    // the `jarvy.tool.installs` counter
                                    // under-counts every nvm/rustup/brew-
                                    // bootstrap install.
                                    telemetry::tool_installed(
                                        name,
                                        version,
                                        "custom",
                                        install_start.elapsed(),
                                    );
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
                    let install_start = std::time::Instant::now();
                    println!(
                        "Installing {} version {} using custom installer",
                        name, version
                    );

                    match tools::add(name, version) {
                        Ok(()) => {
                            println!("Successfully installed {} ({})", name, version);
                            // Round-2 obs F13: same fix as the parallel path.
                            telemetry::tool_installed(
                                name,
                                version,
                                "custom",
                                install_start.elapsed(),
                            );
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
                    if hook.run_with_policy(false).is_err() {
                        return error_codes::HOOK_FAILED;
                    }
                } else if let Some(default_hook) = tools::spec::get_tool_default_hook(tool_name) {
                    // Fall back to tool's built-in default hook. Default
                    // hooks are advisory: failures are warnings, not blockers.
                    println!(
                        "Running default hook for {}: {}",
                        tool_name, default_hook.description
                    );
                    let env = HookEnv::for_tool(tool_name, version);
                    let advisory_settings = HookConfig {
                        continue_on_error: true,
                        ..hook_settings.clone()
                    };
                    let hook = Hook::with_config(
                        default_hook.script,
                        &format!("{} default_hook", tool_name),
                        advisory_settings,
                    )
                    .with_env(env);
                    let _ = hook.run_with_policy(false);
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
    run_packages_phase(&config, file, dry_run);

    // Git configuration
    run_git_phase(&config, dry_run);

    // Environment variable setup
    let env_config = config.get_env();
    let env_settings = &env_config.config;

    if !env_config.vars.is_empty() || !env_config.secrets.is_empty() {
        // Build environment context
        let ctx = EnvContext::new();

        // Collect secrets if any (skip in unattended mode or dry run).
        // Reuse SecretsConfig::default()'s ci_mode (which already
        // routes through sandbox::is_seamless per PRD-053) and OR in
        // dry-run; only fail_on_missing differs from the default.
        let secrets_config = SecretsConfig {
            ci_mode: SecretsConfig::default().ci_mode || dry_run,
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
            if hook.run_with_policy(dry_run).is_err() {
                return error_codes::HOOK_FAILED;
            }
        }
    }

    // Auto-start services if configured
    run_services_phase(&config, file, ci_env.is_some(), dry_run);

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

    // Capture environment state for drift detection. Normally gated
    // on `[drift].enabled` in jarvy.toml, but in seamless mode
    // (sandbox or CI) we auto-baseline on first run when the
    // version_check came back clean — turning a pre-loaded sandbox
    // image into a drift-trackable baseline without the operator
    // running `jarvy drift accept` at image bake time (PRD-053).
    if !dry_run {
        let drift_config = config.drift.clone().unwrap_or_default();
        let project_dir = std::path::Path::new(file)
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let auto_baseline_eligible = !drift_config.enabled
            && crate::sandbox::is_seamless()
            && version_check.needs_install.is_empty()
            && version_check.unknown.is_empty()
            && !crate::paths::state_json(project_dir).exists();
        if drift_config.enabled || auto_baseline_eligible {
            capture_drift_baseline(
                project_dir,
                std::path::Path::new(file),
                &known_tools,
                &drift_config.track_files,
                auto_baseline_eligible,
            );
        }
    }

    // Mark as initialized after successful setup (first-run complete)
    if !dry_run {
        let _ = mark_initialized();
    }

    // Post-install PATH hint: when something was newly installed,
    // remind the user that new binaries / PATH updates dropped by
    // package-manager postscripts only land in *future* shells. The
    // running shell still has the pre-setup PATH. Previously the
    // legacy `refresh_shell()` tried to source `~/.zprofile` and
    // `exec` the user's shell mid-setup; that aborted the whole
    // flow when the user's dotfiles had any syntax incompatibility
    // with `/bin/sh`, and on success replaced the jarvy process so
    // post-install hooks never ran. Plain hint instead.
    if !dry_run && !successfully_installed.is_empty() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "$SHELL".to_string());
        println!("\nTip: open a new terminal or run `exec {shell}` to pick up new PATH entries.");
    }

    // Second chance to surface the telemetry opt-in. The first-run
    // boxed notice in `src/init.rs` is the primary ask, but it only
    // fires when `~/.jarvy/` is created. A user can blow past it
    // (CI, copy-pasted setup) — show a one-liner at the end of
    // every `jarvy setup` until they've made a decision. Stays
    // quiet once `[telemetry] enabled` is either explicitly true
    // or explicitly false (we treat any persisted config as a
    // signal of intent). Stderr so command-output piping is safe.
    if !dry_run {
        emit_telemetry_hint_if_undecided();
    }

    0
}

/// Print a one-line telemetry opt-in nudge on stderr if the user has
/// not yet made an explicit choice. "Explicit" means the
/// `[telemetry]` section exists in `~/.jarvy/config.toml` with
/// `enabled` set either way; absence of the section (the default-
/// shaped first-run config) is treated as "not yet decided" and
/// triggers the nudge.
fn emit_telemetry_hint_if_undecided() {
    use std::fs;

    let Some(home) = dirs::home_dir() else {
        return;
    };
    let config_path = home.join(".jarvy").join("config.toml");
    let Ok(content) = fs::read_to_string(&config_path) else {
        return;
    };
    // A user who set `[telemetry]\nenabled = true|false` has decided.
    // Anything else (no section, or section present without an
    // explicit `enabled = …` line) is treated as undecided.
    let decided = content.lines().any(|l| {
        let t = l.trim();
        t == "enabled = true" || t == "enabled = false"
    });
    if decided {
        return;
    }
    eprintln!(
        "\nTip: Jarvy telemetry is opt-in and currently off. Anonymized usage data helps prioritize fixes.\n     Enable with: jarvy telemetry enable   |   Details: https://jarvy.dev/telemetry/"
    );
}

/// Install language-specific packages (npm, pip, cargo) configured under
/// the `[npm]` / `[pip]` / `[cargo]` sections of `jarvy.toml`. Extracted
/// from `run_setup` (review item 21) — runs after tool install and before
/// git/env configuration.
fn run_packages_phase(config: &Config, file: &str, dry_run: bool) {
    if !config.has_packages() {
        return;
    }
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

/// Apply `[git]` configuration (user identity, signing, aliases, line
/// endings) via `crate::git::GitSetup`. Refusal of `!`-prefixed values is
/// applied inside `GitSetup::set_config`.
fn run_git_phase(config: &Config, dry_run: bool) {
    if !config.has_git() {
        return;
    }
    let Some(git_config) = config.get_git() else {
        return;
    };
    if dry_run {
        println!("\n=== Git Configuration (dry-run) ===");
        if let Some(ref name) = git_config.user_name {
            if let Some(resolved) = name.resolve() {
                println!("[DRY-RUN] Would set git config user.name: {resolved}");
            }
        }
        if let Some(ref email) = git_config.user_email {
            if let Some(resolved) = email.resolve() {
                println!("[DRY-RUN] Would set git config user.email: {resolved}");
            }
        }
        if git_config.signing {
            println!("[DRY-RUN] Would enable commit signing");
            if let Some(ref key) = git_config.signing_key {
                println!("[DRY-RUN] Would set signing key: {key}");
            }
        }
        if let Some(ref branch) = git_config.default_branch {
            println!("[DRY-RUN] Would set init.defaultBranch: {branch}");
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
            Err(e) => eprintln!("Warning: Git configuration failed: {e}"),
        }
    }
}

/// Auto-start services (`docker compose` / `tilt`) if `[services]` is
/// configured to do so for this environment. Containment-checked path
/// resolution lives in `services::detect_backend_with_config`.
fn run_services_phase(config: &Config, file: &str, is_ci: bool, dry_run: bool) {
    let services_config = &config.services;
    if !services_config.should_auto_start(is_ci) {
        return;
    }
    let working_dir = std::path::Path::new(file)
        .parent()
        .unwrap_or(std::path::Path::new("."));

    let Some((backend, config_path)) = services::detect_backend_with_config(
        working_dir,
        services_config.compose_file.as_deref(),
        services_config.tilt_file.as_deref(),
    ) else {
        return;
    };

    let backend_impl = services::get_backend(backend);
    if !backend_impl.is_installed() {
        eprintln!(
            "Note: {backend} config found but {backend} is not installed. \
             Skipping services auto-start."
        );
        return;
    }

    if dry_run {
        println!("\n[DRY-RUN] Would auto-start {backend} services");
    } else {
        println!("\nAuto-starting {backend} services...");
        match backend_impl.start(&config_path, true) {
            Ok(result) => println!("{}", result.message),
            Err(e) => {
                // Services auto-start is advisory — don't fail the setup.
                eprintln!("Warning: Failed to auto-start services: {e}");
            }
        }
    }
}

/// Detect install method for a tool based on its path. Delegates to
/// the canonical classifier in `tools::install_method`. Three other
/// copies were drifting (round-2 maint F1) — they're being migrated
/// onto this one source of truth.
fn detect_install_method(tool: &str) -> String {
    crate::tools::install_method::detect_install_method_for_tool(tool).to_string()
}

/// Capture a drift baseline (`.jarvy/state.json`) for the project.
///
/// `auto` distinguishes the seamless-mode silent auto-baseline (one
/// stderr line, `[jarvy] auto-baselined ...`) from the explicit
/// `[drift].enabled = true` path (full stdout summary). Both write
/// the same on-disk state file.
fn capture_drift_baseline(
    project_dir: &std::path::Path,
    config_path: &std::path::Path,
    known_tools: &[(String, crate::config::Tool)],
    track_files: &[String],
    auto: bool,
) {
    let borrowed: Vec<(&String, &crate::config::Tool)> =
        known_tools.iter().map(|(k, v)| (k, v)).collect();
    capture_drift_baseline_borrowed(project_dir, config_path, &borrowed, track_files, auto)
}

/// Borrow-based variant of `capture_drift_baseline` — lets the
/// verify-only auto-baseline path filter `tool_configs` without
/// deep-cloning every entry. Same on-disk output shape.
fn capture_drift_baseline_borrowed(
    project_dir: &std::path::Path,
    config_path: &std::path::Path,
    known_tools: &[(&String, &crate::config::Tool)],
    track_files: &[String],
    auto: bool,
) {
    let mut state = crate::drift::EnvironmentState::new();
    for (tool_name, tool) in known_tools {
        if let Ok(path) = which::which(tool_name.as_str()) {
            state.set_tool(
                tool_name,
                &tool.version,
                &path,
                &detect_install_method(tool_name),
            );
        }
    }
    for file_path in track_files {
        let full_path = project_dir.join(file_path);
        if full_path.exists()
            && let Ok(hash) = crate::drift::state::hash_file(&full_path)
        {
            state.set_file_hash(file_path, &hash);
        }
    }
    if config_path.exists()
        && let Ok(hash) = crate::drift::state::hash_file(config_path)
    {
        state.set_config_hash(&hash);
    }
    match state.save(project_dir) {
        Err(e) => eprintln!("Warning: Could not save drift detection state: {}", e),
        Ok(()) if auto => {
            tracing::info!(
                event = "drift.baseline.auto_captured",
                tool_count = state.tool_count(),
                provider = %crate::sandbox::detect()
                    .map(|e| e.provider.to_string())
                    .unwrap_or_default(),
                "auto-baselined drift state for seamless mode"
            );
            eprintln!(
                "[jarvy] auto-baselined drift state for seamless mode ({} tools)",
                state.tool_count()
            );
        }
        Ok(()) => {
            tracing::info!(
                event = "drift.baseline.captured",
                tool_count = state.tool_count(),
                "drift detection baseline captured"
            );
            println!(
                "\nDrift detection baseline captured ({} tools)",
                state.tool_count()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    //! Smoke tests for the extracted phase helpers (review item 21 / item 8
    //! follow-up). These run the dry-run path of each phase so we catch
    //! signature drift and panics — not the full install behavior.

    use super::*;

    fn config_from(toml: &str) -> Config {
        Config::from_toml_str(toml).expect("test toml must parse")
    }

    #[test]
    fn run_packages_phase_no_packages_section_is_noop() {
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"
            "#,
        );
        // No assertion needed — just verify no panic on the no-op path.
        run_packages_phase(&cfg, "jarvy.toml", true);
    }

    #[test]
    fn run_packages_phase_dry_run_does_not_invoke_pm() {
        // [npm] / [pip] / [cargo] sections present; dry_run=true must NOT
        // shell out to npm/pip/cargo.
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"

            [npm]
            typescript = "^5.0"
            "#,
        );
        run_packages_phase(&cfg, "jarvy.toml", true);
    }

    #[test]
    fn run_git_phase_no_git_section_is_noop() {
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"
            "#,
        );
        run_git_phase(&cfg, true);
    }

    #[test]
    fn run_git_phase_dry_run_does_not_invoke_git() {
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"

            [git]
            user_name = "Test User"
            user_email = "test@example.com"
            default_branch = "main"

            [git.aliases]
            co = "checkout"
            "#,
        );
        run_git_phase(&cfg, true);
    }

    #[test]
    fn run_services_phase_disabled_is_noop() {
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"
            "#,
        );
        run_services_phase(&cfg, "jarvy.toml", false, true);
    }

    #[test]
    fn run_services_phase_dry_run_with_compose_file_does_not_invoke_docker() {
        // Even though the path won't exist, detect_backend_with_config
        // returns None and the phase exits cleanly.
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"

            [services]
            enabled = true
            auto_start = true
            "#,
        );
        run_services_phase(&cfg, "jarvy.toml", false, true);
    }
}
