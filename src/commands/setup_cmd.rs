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

use crate::chatter;
use crate::ci;
use crate::config::{Config, EnvValue};
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
    profile: bool,
    profile_output: Option<&str>,
) -> i32 {
    // Determine effective parallelism level
    let parallel_jobs = if sequential { 1 } else { jobs.max(1) };

    // `--profile` phase-level timing. Per-tool timings are NOT recorded
    // here: the profiler's start_tool/end_tool model assumes sequential
    // installs, and the default install path is parallel (PRD-001).
    // Phase durations + the existing per-tool `duration_ms` telemetry
    // events cover the same question without a racy API.
    let mut profiler = if profile {
        crate::observability::Profiler::new()
    } else {
        crate::observability::Profiler::disabled()
    };

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

    let mut config = Config::new_with_workspace(&config_path);
    // Tag the AI hooks block as remote-origin when the user supplied
    // `--from <url>`. The runner refuses raw `command = "..."` entries
    // from remote configs even if `allow_custom_commands = true` —
    // remote configs can narrow but not broaden policy.
    if from.is_some() {
        config.mark_remote();
    }
    let hooks_config = config.get_hooks();
    let hook_settings = HookConfig::from(&hooks_config.config);

    // Set the global default for sudo usage based on config
    tools::set_default_use_sudo(config.use_sudo());

    // Execute pre_setup hook if configured. Routed through the shared
    // Hook::run_with_policy helper so the four-line "fail-or-continue"
    // refrain isn't repeated 4× in this function (maintainability
    // review #11).
    if !no_hooks && let Some(ref script) = hooks_config.pre_setup {
        let hook = Hook::with_config(script, "pre_setup", hook_settings.clone())
            .with_env(HookEnv::global());
        if hook.run_with_policy(dry_run).is_err() {
            return error_codes::HOOK_FAILED;
        }
    }

    if !dry_run {
        setup();
    } else {
        println!("[DRY-RUN] Would run platform setup");
    }

    // Get tool configs with role override if --role flag was used
    if let Some(role_name) = role {
        chatter!("Using role override: {}", role_name);
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
    profiler.start_phase("version_check");
    chatter!("Checking tool versions...");
    let version_check = tools::spec::check_tools_parallel(
        tool_configs
            .values()
            .map(|t| (t.name.as_str(), t.version.as_str())),
    );

    // Report version check results
    chatter!("{}", version_check.summary_string());

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
                capture_drift_baseline_borrowed(
                    project_dir,
                    std::path::Path::new(file),
                    tool_configs
                        .iter()
                        .filter(|(_, t)| tools::get_tool(&t.name).is_some()),
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
        chatter!(
            "Already installed: {}",
            version_check
                .satisfied
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Log unknown tools — telemetry is the canonical request channel
    // (no GitHub account needed, zero triage on the maintainer side).
    // The fallback URL is shown only when telemetry is off. Seamless
    // mode (PRD-053) suppresses the "enable telemetry" hint because
    // those environments are typically multi-tenant or ephemeral and
    // the operator can't toggle telemetry per-run — but the channel
    // selection itself depends only on telemetry state, never on
    // seamless. (Conflating the two led to a real bug where the
    // renderer claimed "Reported via telemetry" while telemetry was
    // disabled and nothing was actually sent.)
    let seamless = crate::sandbox::is_seamless();
    let unsupported_channel = tools::unsupported::pick_channel(telemetry::is_enabled());

    for (name, version) in &version_check.unknown {
        let report = tools::unsupported::build_report(name, Some(version), unsupported_channel);
        // Human-readable block: name, suggestions, channel status,
        // scaffold. The renderer hides the GitHub URL when telemetry
        // covers the request; in seamless mode the "enable telemetry"
        // hint is suppressed even on the Manual branch.
        eprint!(
            "{}",
            tools::unsupported::to_human(&report, unsupported_channel, seamless)
        );
        // Single canonical `tool.unsupported` event — uniform field
        // shape across the setup and `--request` paths so log queries
        // return one consistent table. See CLAUDE.md "Event Taxonomy".
        //
        // `suggestions` is emitted as a comma-joined string (NOT Debug
        // format) so the JSON file layer produces a usable scalar that
        // downstream consumers (Loki / jq) can split. The Debug form
        // emitted a quoted Rust-Debug blob.
        //
        // `fallback_issue_url` is included only when the channel is
        // manual — the URL bloats every log line otherwise and is by-
        // design unused when telemetry covered the request.
        let version_str = report.version.as_deref().unwrap_or("");
        let suggestions_csv = report.suggestions.join(",");
        if matches!(
            unsupported_channel,
            tools::unsupported::RequestChannel::Manual
        ) {
            tracing::warn!(
                event = "tool.unsupported",
                tool = %report.tool,
                version = %version_str,
                source = %telemetry::Source::Config,
                platform = %std::env::consts::OS,
                suggestions = %suggestions_csv,
                channel = %report.channel,
                fallback_issue_url = %report.fallback_issue_url,
                scaffold_cmd = %report.scaffold_cmd,
                exit_code = report.exit_code,
                "tool not in registry"
            );
        } else {
            tracing::warn!(
                event = "tool.unsupported",
                tool = %report.tool,
                version = %version_str,
                source = %telemetry::Source::Config,
                platform = %std::env::consts::OS,
                suggestions = %suggestions_csv,
                channel = %report.channel,
                scaffold_cmd = %report.scaffold_cmd,
                exit_code = report.exit_code,
                "tool not in registry"
            );
        }
        // Use the sanitized name from the report — the raw `name` from
        // `version_check.unknown` is attacker-controlled (jarvy.toml
        // keys can contain any bytes the TOML parser accepts).
        // Routing it directly into a `KeyValue` attribute would forward
        // control bytes and high-cardinality strings to the OTLP
        // collector. See security review F4 (round 2).
        telemetry::tool_not_supported(
            &report.tool,
            report.version.as_deref(),
            telemetry::Source::Config,
        );
    }

    // If every configured tool was unknown — nothing to install, nothing
    // already satisfied — the run produced zero work. Signal that with
    // exit code TOOL_UNSUPPORTED so AI agents driving setup know to act
    // (file a request, scaffold the tool, or revise the config). Mixed
    // runs (some known + some unknown) keep returning 0 so partial
    // setups still succeed.
    if !version_check.unknown.is_empty()
        && version_check.needs_install.is_empty()
        && version_check.satisfied.is_empty()
    {
        return error_codes::TOOL_UNSUPPORTED;
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
            if let Some((_eco, cmd)) = crate::tools::fallback::preview_route(name, version) {
                println!(
                    "[DRY-RUN] Would install {} via fallback route: `{}`",
                    name, cmd
                );
            } else {
                println!(
                    "[DRY-RUN] Would install {} version {} using custom installer",
                    name, version
                );
            }
        }
    } else {
        // Emit setup_started event
        profiler.start_phase("tool_install");
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
                    // Batch install failed entirely.
                    // Discriminate `Unsupported` (no platform install
                    // method — emit `tool.unsupported` not
                    // `tool.failed`) from real install failures
                    // (Observability F1).
                    let kind = e.kind();
                    if e.is_no_platform_installer() {
                        for (tool_name, _, version) in packages {
                            let (fallback_declared, fallback_blocked) =
                                tools::fallback::unsupported_info(tool_name);
                            tracing::info!(
                                event = "tool.unsupported",
                                tool = %tool_name,
                                version = %version,
                                source = "config",
                                channel = "registered_no_platform_installer",
                                platform = std::env::consts::OS,
                                exit_code = error_codes::TOOL_UNSUPPORTED,
                                fallback_declared,
                                fallback_blocked,
                            );
                            telemetry::tool_not_supported(
                                tool_name,
                                Some(version),
                                telemetry::Source::Config,
                            );
                            eprintln!(
                                "  {} has no installer on this platform; skipping.",
                                tool_name
                            );
                        }
                    } else {
                        for (tool_name, _, version) in packages {
                            let msg =
                                format!("Failed to install {} ({}): {:?}", tool_name, version, e);
                            eprintln!("{}", msg);
                            telemetry::tool_failed_with_kind(
                                tool_name,
                                version,
                                kind,
                                &format!("{:?}", e),
                            );
                        }
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
                            // Same discrimination as the batch path:
                            // route `Unsupported` to `tool.unsupported`
                            // so it doesn't pollute the failed-installs
                            // counter on Windows when a tool ships with
                            // no winget manifest (Observability F1).
                            if e.is_no_platform_installer() {
                                let (fallback_declared, fallback_blocked) =
                                    tools::fallback::unsupported_info(name);
                                tracing::info!(
                                    event = "tool.unsupported",
                                    tool = %name,
                                    version = %version,
                                    source = "config",
                                    channel = "registered_no_platform_installer",
                                    platform = std::env::consts::OS,
                                    exit_code = error_codes::TOOL_UNSUPPORTED,
                                    fallback_declared,
                                    fallback_blocked,
                                );
                                telemetry::tool_not_supported(
                                    name,
                                    Some(version),
                                    telemetry::Source::Config,
                                );
                                eprintln!(
                                    "  {} has no installer on this platform; skipping.",
                                    name
                                );
                            } else {
                                let msg =
                                    format!("Failed to install {} ({}): {:?}", name, version, e);
                                eprintln!("{}", msg);
                                telemetry::tool_failed_with_kind(
                                    name,
                                    version,
                                    e.kind(),
                                    &format!("{:?}", e),
                                );
                            }
                        }
                    }
                }
            }
        }

        // Execute hooks for successfully installed tools
        profiler.start_phase("tool_hooks");
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
    profiler.start_phase("packages");
    run_packages_phase(&config, file, dry_run);

    // Git configuration
    profiler.start_phase("git_config");
    run_git_phase(&config, dry_run);

    // Git hook framework (pre-commit / husky / lefthook) — PRD-048
    profiler.start_phase("git_hooks");
    run_git_hooks_phase(&config, file, dry_run);

    // AI agent hook provisioning (Claude Code, Cursor, Codex, Windsurf, ...)
    profiler.start_phase("ai_hooks");
    run_ai_hooks_phase(&config, dry_run);

    // MCP server registration — auto-register `jarvy mcp` with each
    // configured agent so terminal AIs can discover Jarvy's tools.
    profiler.start_phase("mcp_register");
    run_mcp_register_phase(&config, dry_run);

    // Continuous discovery — advisory only. Runs `jarvy discover`'s
    // analyzer against the project tree and reports any new tools
    // not already in `[provisioner]`. NEVER mutates jarvy.toml from
    // setup; users must explicitly run `jarvy discover --apply`.
    profiler.start_phase("discover");
    run_continuous_discover_phase(file);

    // Environment variable setup
    profiler.start_phase("env_setup");
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

        // Merge vars and secrets for the dotenv path (flat strings).
        let mut all_vars: HashMap<String, String> = env_config
            .vars
            .iter()
            .map(|(k, v)| (k.clone(), v.value().to_string()))
            .collect();
        all_vars.extend(secrets.clone());

        // Shell path keeps `EnvValue` so append/position metadata reaches
        // the rc-file writer. Secrets are always simple strings.
        let mut all_shell_vars: HashMap<String, EnvValue> = env_config.vars.clone();
        for (k, v) in secrets {
            all_shell_vars.insert(k, EnvValue::Simple(v));
        }

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
                let preview = preview_shell_rc(shell, &all_shell_vars, &ctx);
                println!("{}", preview);
            } else {
                match update_shell_rc(shell, &all_shell_vars, &ctx, &shell_config) {
                    Ok(path) => println!("Updated shell rc at {}", path.display()),
                    Err(e) => eprintln!("Warning: Could not update shell rc: {}", e),
                }
            }
        }
    }

    // Execute post_setup hook if configured
    if !no_hooks && let Some(ref script) = hooks_config.post_setup {
        let hook = Hook::with_config(script, "post_setup", hook_settings.clone())
            .with_env(HookEnv::global());
        if hook.run_with_policy(dry_run).is_err() {
            return error_codes::HOOK_FAILED;
        }
    }

    // Auto-start services if configured
    profiler.start_phase("services");
    run_services_phase(&config, file, ci_env.is_some(), dry_run);

    // Dotfiles phase — clone/apply personal dotfile repo via chezmoi/yadm.
    profiler.start_phase("dotfiles");
    run_dotfiles_phase(&config, dry_run);

    // Adoption denominator for agent profiles (PRD-058): emit a
    // debug event when no profiles.json exists so the feature-reach
    // funnel has a baseline. Mirrors dotfiles.phase_absent. Best-
    // effort — path errors are swallowed; setup must never fail here.
    if crate::observability::telemetry_gate::is_enabled() {
        let profiles_json_absent = crate::paths::agent_profiles_dir()
            .ok()
            .map(|d| !d.join("profiles.json").exists())
            .unwrap_or(false);
        if profiles_json_absent {
            tracing::debug!(
                event = "agent_profile.not_configured",
                "agent profiles not configured"
            );
        }
    }
    run_agent_profile_hint_phase(&config);

    if config.has_hooks() && !no_hooks {
        chatter!("\nHooks execution summary:");
        if hooks_config.pre_setup.is_some() {
            chatter!("  - pre_setup: executed");
        }
        let tool_hooks_count = hooks_config
            .tool_hooks
            .values()
            .filter(|h| h.post_install.is_some())
            .count();
        if tool_hooks_count > 0 {
            chatter!("  - tool post_install hooks: {} executed", tool_hooks_count);
        }
        if hooks_config.post_setup.is_some() {
            chatter!("  - post_setup: executed");
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
        let auto_baseline_eligible_now = auto_baseline_eligible(
            drift_config.enabled,
            crate::sandbox::is_seamless(),
            version_check.needs_install.is_empty(),
            version_check.unknown.is_empty(),
            crate::paths::state_json(project_dir).exists(),
        );
        if drift_config.enabled || auto_baseline_eligible_now {
            // Remote configs cannot broaden trust: refuse the
            // track_files list from a remote origin unless the user
            // explicitly opted in via `[drift] allow_remote = true`.
            // Empty slice = still capture tool state (safe — bounded
            // to the tool registry) but no arbitrary file hashing.
            let track_files: &[String] = if config.origin == crate::ai_hooks::ConfigOrigin::Remote
                && !drift_config.allow_remote
            {
                if !drift_config.track_files.is_empty()
                    && crate::observability::telemetry_gate::is_enabled()
                {
                    tracing::warn!(
                        event = "drift.remote_refused",
                        reason = "allow_remote_not_set",
                        refused_track_files = drift_config.track_files.len(),
                    );
                }
                &[]
            } else {
                drift_config.track_files.as_slice()
            };
            capture_drift_baseline(
                project_dir,
                std::path::Path::new(file),
                &known_tools,
                track_files,
                auto_baseline_eligible_now,
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
        chatter!("\nTip: open a new terminal or run `exec {shell}` to pick up new PATH entries.");
    }

    // Second chance to surface the telemetry opt-out. The first-run
    // boxed notice in `src/init.rs` is the primary disclosure, but it
    // only fires when `~/.jarvy/` is created. A user can blow past it
    // (CI, copy-pasted setup) — show a one-liner at the end of
    // every `jarvy setup` until they've made a decision. Stays
    // quiet once `[telemetry] enabled` is either explicitly true
    // or explicitly false (we treat any persisted config as a
    // signal of intent). Stderr so command-output piping is safe.
    if !dry_run {
        emit_telemetry_hint_if_undecided();
    }

    // Freshness advisory — read cache (never network) and spawn
    // detached background refresher. Deliberately last real work
    // before the profiler report so setup latency is unaffected.
    if !dry_run {
        let maintenance_state = run_maintenance_phase(&config, file);
        // `setup_cmd` doesn't emit `setup.completed` today (only the
        // legacy `src/setup.rs` does), so bridge the adoption
        // denominator PMs need (analytics F2) via a focused event.
        // Bounded label; single emission per setup invocation.
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(
                event = "setup.maintenance_evaluated",
                maintenance_state = maintenance_state.as_str(),
            );
        }
    }

    // `--profile` report goes to stderr so stdout stays clean for
    // command-output piping (same rule as the tracing console layers).
    if profiler.is_enabled() {
        profiler.end_phase();
        let report = profiler.report();
        eprint!("{}", report.to_summary());
        if let Some(path) = profile_output {
            match report.to_json_file(path) {
                Ok(()) => eprintln!("Profile written to {path}"),
                Err(e) => eprintln!("Warning: could not write profile to {path}: {e}"),
            }
        }
    }

    0
}

/// Print a one-line telemetry opt-out nudge on stderr if the user has
/// not yet made an explicit choice. "Explicit" means the
/// `[telemetry]` section exists in `~/.jarvy/config.toml` with
/// `enabled` set either way; absence of the section (the default-
/// shaped first-run config) is treated as "not yet decided" and
/// triggers the nudge.
fn emit_telemetry_hint_if_undecided() {
    use std::fs;

    // Only nudge when telemetry is actually on right now. If the user
    // ran with `JARVY_TELEMETRY=0` or is inside an auto-disabled
    // sandbox/CI, telling them how to opt out is just noise.
    if !crate::telemetry::is_enabled() {
        return;
    }

    let Some(home) = dirs::home_dir() else {
        return;
    };
    let config_path = home.join(".jarvy").join("config.toml");
    let content = fs::read_to_string(&config_path).unwrap_or_default();
    if crate::telemetry::user_decided(&content) {
        return;
    }
    eprintln!(
        "\nNote: Jarvy telemetry is opt-out and currently on. Anonymized usage data helps prioritize fixes.\n      Disable with: jarvy telemetry disable   |   Details: https://jarvy.dev/telemetry/"
    );
    crate::telemetry::undecided_nudge_shown();
}

/// Render a sorted, scope-labelled package list for the dry-run preview.
///
/// Operators need three things from the preview: (1) which ecosystem,
/// (2) where the install will land (project-local vs user-global vs
/// machine-global), (3) which packages by name. All four ecosystems
/// emit the same shape so a multi-ecosystem `jarvy.toml` previews
/// consistently.
///
/// The `.NET global tool` label is kept verbatim for backward
/// compatibility with the existing `examples_validation` regression
/// test that pins the announcement string.
fn preview_packages<'a, I>(ecosystem: &str, scope_label: &str, names: I)
where
    I: IntoIterator<Item = &'a str>,
{
    print!("{}", render_package_preview(ecosystem, scope_label, names));
}

/// Pure-function version of the dry-run preview body — returns the
/// formatted output as a `String` so unit tests can pin the empty /
/// single / plural / `.NET global tool` branches without capturing
/// stdout. The wrapping `preview_packages` simply prints the result.
pub(crate) fn render_package_preview<'a, I>(ecosystem: &str, scope_label: &str, names: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let mut names: Vec<&str> = names.into_iter().collect();
    names.sort_unstable();
    let count = names.len();
    let (label, command_hint) = match ecosystem {
        ".NET global tool" => (
            ".NET global tool(s)",
            " via `dotnet tool update -g`".to_string(),
        ),
        other => (
            if count == 1 { "package" } else { "packages" },
            format!(" via `{} install`", other),
        ),
    };
    let mut out = String::with_capacity(64 + count * 32);
    use std::fmt::Write;
    let _ = writeln!(
        out,
        "[DRY-RUN] Would install {} {}{} ({})",
        count, label, command_hint, scope_label
    );
    // Sanitize package names before printing — a hostile `jarvy.toml`
    // can land ANSI / OSC / Trojan-Source bidi sequences in TOML quoted
    // keys; the dry-run preview is the path operators trust as "safe to
    // inspect untrusted configs," so this is the trust boundary, not
    // the install loop.
    for name in names {
        let _ = writeln!(
            out,
            "[DRY-RUN]   - {}",
            crate::observability::redact_for_display(name)
        );
    }
    out
}

/// Install language-specific packages (npm, pip, cargo, nuget) configured
/// under the `[npm]` / `[pip]` / `[cargo]` / `[nuget]` sections of
/// `jarvy.toml`. Extracted from `run_setup` (review item 21) — runs after
/// tool install and before git/env configuration.
///
/// Wraps the phase in a `tracing::info_span!` carrying `dry_run` and
/// per-backend booleans so file-logging captures the path the user took,
/// and emits `packages.phase_started` / `packages.phase_completed`
/// structured events so `jarvy ticket` bundles show whether dry-run was
/// honored even when stdout was redirected.
fn run_packages_phase(config: &Config, file: &str, dry_run: bool) {
    let telemetry_on = crate::observability::telemetry_gate::is_enabled();
    let packages_ref = config.packages_ref();
    let project_dir = std::path::Path::new(file)
        .parent()
        .unwrap_or(std::path::Path::new("."));

    let has_npm = packages_ref.npm.is_some();
    let has_pip = packages_ref.pip.is_some();
    let has_cargo = packages_ref.cargo.is_some();
    let has_nuget = packages_ref.nuget.is_some();
    let has_gem = packages_ref.gem.is_some();
    let has_go = packages_ref.go.is_some();
    let backend_count = (has_npm as u32)
        + (has_pip as u32)
        + (has_cargo as u32)
        + (has_nuget as u32)
        + (has_gem as u32)
        + (has_go as u32);

    let _span = tracing::info_span!(
        "packages",
        dry_run = %dry_run,
        npm = %has_npm,
        pip = %has_pip,
        cargo = %has_cargo,
        nuget = %has_nuget,
        gem = %has_gem,
        go = %has_go,
    )
    .entered();

    // Emit phase_started BEFORE the early return so an operator
    // querying "did dry-run reach the packages phase?" can answer from
    // logs alone (Obs F8) — without this, a config with zero
    // `[npm]/[pip]/[cargo]/[nuget]` blocks was indistinguishable from
    // a mid-phase crash.
    if telemetry_on {
        tracing::info!(
            event = "packages.phase_started",
            dry_run,
            backend_count,
            npm = has_npm,
            pip = has_pip,
            cargo = has_cargo,
            nuget = has_nuget,
            gem = has_gem,
            go = has_go,
        );
    }

    if !config.has_packages() {
        if telemetry_on {
            tracing::info!(
                event = "packages.phase_skipped",
                reason = "no_packages_configured",
                dry_run,
            );
        }
        return;
    }

    let started = std::time::Instant::now();

    if dry_run {
        println!("\n=== Package Dependencies (dry-run) ===");
        // Structured event so CI / log scrapers can verify dry-run was
        // honored without parsing stdout. Carries the package counts
        // per ecosystem so dashboards can graph dry-run preview volume.
        // Renamed from `packages.dry_run` to follow the
        // `<domain>.<verb_past_tense>` convention used by the rest of
        // the taxonomy (Obs F5).
        if telemetry_on {
            tracing::info!(
                event = "packages.phase_previewed",
                npm_count = packages_ref.npm.map(|c| c.packages.len()).unwrap_or(0),
                pip_count = packages_ref.pip.map(|c| c.packages.len()).unwrap_or(0),
                cargo_count = packages_ref.cargo.map(|c| c.packages.len()).unwrap_or(0),
                nuget_count = packages_ref.nuget.map(|c| c.packages.len()).unwrap_or(0),
                gem_count = packages_ref.gem.map(|c| c.packages.len()).unwrap_or(0),
                go_count = packages_ref.go.map(|c| c.packages.len()).unwrap_or(0),
            );
        }
        // Symmetric preview across all four ecosystems: announce the
        // count + scope label, then list each package by name so the
        // operator can review what will land BEFORE the real run.
        // Maintainability F5: nuget had this fidelity, npm/pip/cargo
        // were one-liners — the asymmetry was misleading.
        if let Some(npm) = packages_ref.npm {
            preview_packages(
                "npm",
                "project-local",
                npm.packages.keys().map(String::as_str),
            );
        }
        if let Some(pip) = packages_ref.pip {
            preview_packages(
                "pip",
                "project-local",
                pip.packages.keys().map(String::as_str),
            );
        }
        if let Some(cargo) = packages_ref.cargo {
            preview_packages(
                "cargo",
                "user-global",
                cargo.packages.keys().map(String::as_str),
            );
        }
        if let Some(nuget) = packages_ref.nuget {
            preview_packages(
                ".NET global tool",
                "machine-global",
                nuget.packages.keys().map(String::as_str),
            );
        }
        if let Some(gem) = packages_ref.gem {
            preview_packages(
                "gem",
                "ruby-global",
                gem.packages.keys().map(String::as_str),
            );
        }
        if let Some(go) = packages_ref.go {
            preview_packages("go", "GOBIN", go.packages.keys().map(String::as_str));
        }
    } else {
        println!("\n=== Installing Package Dependencies ===");
        if let Err(e) = packages::install_packages(packages_ref, project_dir) {
            // Ecosystem-level failure is `error!` (the entire phase
            // is broken — e.g. venv creation failed before any
            // package was attempted). Per-package failures stay
            // `warn!` in cargo_pkg/nuget. (Obs F7 — level inversion
            // fix.)
            if telemetry_on {
                tracing::error!(
                    event = "packages.install_failed",
                    error_kind = e.kind(),
                    error = %e,
                );
            }
            eprintln!("Warning: Package installation failed: {}", e);
        }
    }

    if telemetry_on {
        tracing::info!(
            event = "packages.phase_completed",
            dry_run,
            backend_count,
            duration_ms = started.elapsed().as_millis() as u64,
        );
    }
}

/// Decision for the git-config phase, extracted so the remote-origin trust gate
/// is unit-testable without running git. See `resolve_git_phase`.
// Transient, constructed once per setup run and immediately consumed — the
// size gap between the data-carrying `Apply` and the empty variants is not
// worth a `Box` indirection here.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum GitPhaseDecision {
    /// No `[git]` block — nothing to do.
    Skip,
    /// Remote-origin config without `allow_remote` — refused (no writes).
    Refused,
    /// Apply this config. `scope` is already forced to `Local` for a remote
    /// origin, so a remote can never touch `~/.gitconfig`.
    Apply {
        config: crate::git::GitConfig,
        is_remote: bool,
    },
}

/// Resolve the git-config phase decision. Pure (no git I/O): applies the
/// remote-origin trust gate and forces `--local` scope for an authorized remote
/// config. Mirrors `run_git_hooks_phase` / `[packages] allow_remote`.
pub(crate) fn resolve_git_phase(config: &Config) -> GitPhaseDecision {
    if !config.has_git() {
        return GitPhaseDecision::Skip;
    }
    let Some(git_config) = config.get_git() else {
        return GitPhaseDecision::Skip;
    };
    let is_remote = config.origin == crate::ai_hooks::ConfigOrigin::Remote;
    if is_remote && !git_config.allow_remote {
        return GitPhaseDecision::Refused;
    }
    let mut git_config = git_config.clone();
    if is_remote {
        git_config.scope = crate::git::ConfigScope::Local;
    }
    GitPhaseDecision::Apply {
        config: git_config,
        is_remote,
    }
}

/// Apply `[git]` configuration (user identity, signing, aliases, line endings,
/// os-defaults, `[git.extra]`) via `crate::git::GitSetup`. Value refusals are
/// enforced inside `GitSetup`; the remote-origin trust gate is `resolve_git_phase`.
fn run_git_phase(config: &Config, dry_run: bool) {
    let (git_config, is_remote) = match resolve_git_phase(config) {
        GitPhaseDecision::Skip => return,
        GitPhaseDecision::Refused => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "git_config.remote_refused",
                    reason = "allow_remote_not_set",
                    "refused `[git]` configuration from a remote-origin config"
                );
            }
            eprintln!(
                "Warning: skipping [git] configuration from a remote config.\n  \
                 Set `[git] allow_remote = true` in the source config — or copy it locally —\n  \
                 to authorize git configuration from this origin."
            );
            return;
        }
        GitPhaseDecision::Apply { config, is_remote } => (config, is_remote),
    };

    if dry_run {
        println!("\n=== Git Configuration (dry-run) ===");
        if let Some(ref name) = git_config.user_name
            && let Some(resolved) = name.resolve()
        {
            println!("[DRY-RUN] Would set git config user.name: {resolved}");
        }
        if let Some(ref email) = git_config.user_email
            && let Some(resolved) = email.resolve()
        {
            println!("[DRY-RUN] Would set git config user.email: {resolved}");
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
        // Preview the OS-aware / recommended defaults and every `[git.extra]`
        // key — the highest-risk writes to review before applying a config.
        // `[git.extra]` runs the SAME guard gauntlet as a real apply (via
        // extra_write_plan) so the preview cannot claim a key would be set that
        // the real run refuses.
        let setup = crate::git::GitSetup::new(git_config.clone());
        for (key, value) in setup.os_default_plan() {
            println!("[DRY-RUN] Would set git config {key}: {value}");
        }
        match setup.extra_write_plan() {
            Ok(plan) => {
                for (key, value) in plan {
                    println!("[DRY-RUN] Would set git config {key}: {value}");
                }
            }
            Err(e) => println!("[DRY-RUN] [git.extra] would be refused: {e}"),
        }
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(
                event = "git_config.phase_previewed",
                remote = is_remote,
                scope = ?git_config.scope,
                os_defaults_enabled = git_config.os_defaults.unwrap_or(true),
                extra_key_count = git_config.extra.len(),
            );
        }
    } else {
        chatter!("\n=== Git Configuration ===");
        let started = std::time::Instant::now();
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(
                event = "git_config.phase_started",
                remote = is_remote,
                scope = ?git_config.scope,
            );
        }
        let setup = crate::git::GitSetup::new(git_config.clone());
        let result = setup.configure();
        let ok = result.is_ok();
        let error_kind = result.as_ref().err().map_or("none", |e| e.kind());
        match result {
            Ok(()) => chatter!("Git configuration applied successfully"),
            Err(ref e) => eprintln!("Warning: Git configuration failed: {e}"),
        }
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(
                event = "git_config.phase_completed",
                ok,
                error_kind,
                remote = is_remote,
                scope = ?git_config.scope,
                signing = git_config.signing,
                aliases = git_config.aliases.len(),
                extra_key_count = git_config.extra.len(),
                os_defaults_enabled = git_config.os_defaults.unwrap_or(true),
                duration_ms = started.elapsed().as_millis() as u64,
            );
        }
    }
}

/// Auto-install git hook framework during `jarvy setup` (PRD-048).
///
/// Skipped silently when:
/// - `[git_hooks]` block absent
/// - `[git_hooks] enabled = false`
/// - `[git_hooks] auto_install = false`
/// - no framework detected (no `.pre-commit-config.yaml`, etc.)
/// - origin is `Remote` and `allow_remote = false` (trust gate; logs
///   `git_hooks.remote_refused` for audit)
///
/// Failures are advisory — surface a warning but don't fail the whole
/// setup. The dedicated `jarvy hooks install` command exists for users
/// who want a hard-fail on hook install errors.
fn run_git_hooks_phase(config: &Config, file: &str, dry_run: bool) {
    let Some(ref gh_cfg) = config.git_hooks else {
        return;
    };
    if !gh_cfg.enabled || !gh_cfg.auto_install {
        return;
    }
    let project_dir = std::path::Path::new(file)
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();

    // Remote-config trust gate. Mirrors `[packages] allow_remote` —
    // remote configs may narrow trust but cannot install arbitrary
    // git hooks on the consuming machine without explicit opt-in in
    // the source config.
    if config.origin == crate::ai_hooks::ConfigOrigin::Remote && !gh_cfg.allow_remote {
        tracing::warn!(
            event = "git_hooks.remote_refused",
            reason = "allow_remote_not_set",
        );
        eprintln!(
            "\n  Refusing to install git hooks from a remote config (`jarvy setup --from <url>`).\n  \
             Set `[git_hooks] allow_remote = true` in the source config — or copy it locally —\n  \
             to authorize hook installation from this origin."
        );
        return;
    }

    let telemetry_on = crate::observability::telemetry_gate::is_enabled();
    let _span = tracing::info_span!("git_hooks", dry_run = %dry_run).entered();

    if dry_run {
        println!("\n=== Git Hooks (dry-run) ===");
        let framework = gh_cfg
            .framework
            .or_else(|| crate::git_hooks::detect_framework(&project_dir));
        match framework {
            Some(f) => println!("[DRY-RUN] Would install git hooks via {}", f.as_str()),
            None => println!("[DRY-RUN] No hook framework detected — nothing to install"),
        }
        if telemetry_on {
            tracing::info!(
                event = "git_hooks.phase_previewed",
                framework = framework.map(|f| f.as_str()).unwrap_or("none"),
            );
        }
        return;
    }

    chatter!("\n=== Git Hooks ===");
    let started = std::time::Instant::now();
    match crate::git_hooks::install_hooks(gh_cfg, &project_dir) {
        Ok(true) => {
            chatter!("  Git hooks installed");
            if gh_cfg.auto_update
                && let Err(e) = crate::git_hooks::update_hooks(gh_cfg, &project_dir)
            {
                eprintln!("  Warning: hook autoupdate failed: {e}");
            }
            if gh_cfg.run_after_install
                && let Err(e) = crate::git_hooks::run_hooks(gh_cfg, &project_dir, true, None)
            {
                eprintln!("  Warning: initial hook run reported failures: {e}");
            }
            if telemetry_on {
                tracing::info!(
                    event = "git_hooks.phase_completed",
                    installed = true,
                    duration_ms = started.elapsed().as_millis() as u64,
                );
            }
        }
        Ok(false) => {
            chatter!("  No hook framework detected — skipping");
            if telemetry_on {
                tracing::info!(
                    event = "git_hooks.phase_skipped",
                    reason = "no_framework_detected",
                );
            }
        }
        Err(e) => {
            eprintln!("  Warning: git hook install failed: {e}");
            if telemetry_on {
                tracing::warn!(
                    event = "git_hooks.install_failed",
                    error_kind = e.kind(),
                    error = %e,
                );
            }
        }
    }
}

/// Apply `[ai_hooks]` configuration: write Claude Code / Cursor / Codex /
/// Windsurf / Cline / Continue hook settings. Library hooks always
/// apply; raw `command = "..."` entries are refused unless
/// `allow_custom_commands = true` AND the config came from a local file
/// (the remote-origin trust boundary).
///
/// Per-agent failures DO NOT abort the phase — every agent gets a chance
/// to apply. Each success / failure produces its own telemetry event so
/// on-call can distinguish "Cline broke on Windows" from "AI hooks
/// broke" without reading source.
fn run_ai_hooks_phase(config: &Config, dry_run: bool) {
    let Some(ref ai_cfg) = config.ai_hooks else {
        return;
    };
    if ai_cfg.is_empty() {
        return;
    }
    let agent_count = ai_cfg.unique_agents().len();
    let hooks_count = ai_cfg.hooks.len();
    let scope_label = match ai_cfg.scope {
        crate::ai_hooks::HookScope::User => "user",
        crate::ai_hooks::HookScope::Project => "project",
    };

    let _span = tracing::info_span!(
        "ai_hooks",
        agents = %agent_count,
        scope = %scope_label,
        dry_run = %dry_run,
    )
    .entered();

    if dry_run {
        println!("\n=== AI Hooks (dry-run) ===");
        println!(
            "[DRY-RUN] Would provision {} hook(s) for: {:?}",
            hooks_count,
            ai_cfg.unique_agents()
        );
        crate::telemetry::ai_hook_phase_started(agent_count, hooks_count, scope_label, true);
        return;
    }

    chatter!("\n=== AI Hooks ===");
    let started = std::time::Instant::now();
    crate::telemetry::ai_hook_phase_started(agent_count, hooks_count, scope_label, false);

    match crate::ai_hooks::apply(ai_cfg) {
        Ok(report) => {
            chatter!(
                "  Applied {} hook(s) across {} agent(s)",
                report.total_applied(),
                report.successes.len()
            );
            for outcome in &report.successes {
                chatter!("    {:<13} {}", outcome.agent, outcome.path.display());
                for w in &outcome.warnings {
                    chatter!("      warning: {w}");
                }
                crate::telemetry::ai_hook_agent_applied(
                    outcome.agent,
                    outcome.applied,
                    outcome.warnings.len(),
                    &outcome.path,
                );
            }
            for (target, e) in &report.failures {
                eprintln!(
                    "    {:<13} FAILED ({}): {} — other agents still applied",
                    target.slug(),
                    e.kind(),
                    e
                );
                crate::telemetry::ai_hook_agent_failed(target.slug(), e.kind());
            }
            if !report.refused_custom.is_empty() {
                chatter!(
                    "  Refused {} custom hook(s) (set allow_custom_commands = true to apply)",
                    report.refused_custom.len()
                );
            }
            if !report.remote_refused_custom.is_empty() {
                chatter!(
                    "  Refused {} custom hook(s) from remote-fetched config (trust boundary)",
                    report.remote_refused_custom.len()
                );
            }
            crate::telemetry::ai_hook_custom_refused_summary(
                report.refused_custom.len(),
                report.remote_refused_custom.len(),
            );
            crate::telemetry::ai_hook_phase_completed(
                report.total_applied(),
                report.agents_touched(),
                report.refused_custom.len(),
                report.remote_refused_custom.len(),
                report.failures.len(),
                started.elapsed(),
            );
        }
        Err(e) => {
            eprintln!("  Warning: AI hook provisioning failed: {e}");
            crate::telemetry::ai_hook_agent_failed("global", e.kind());
            crate::telemetry::ai_hook_phase_completed(0, 0, 0, 0, 1, started.elapsed());
        }
    }
}

/// Apply `[mcp_register]` configuration: announce the Jarvy MCP server
/// (and any opt-in custom servers) to each developer's AI agents so
/// they can discover and call Jarvy's tools without manual setup.
///
/// Default-on: when `config.mcp_register` is absent, auto-detect which
/// agents the user already has installed (via
/// `crate::mcp_register::auto_detect_agents`) and synthesize a
/// minimal opt-out config (built-in `jarvy` server only, user scope,
/// no custom servers). Skip the auto-default in dry-run, test mode,
/// unattended CI / AI sandboxes, and when the user has set
/// `JARVY_MCP_REGISTER=0`. A one-line stderr disclosure surfaces the
/// agents that landed so the developer sees what was written and how
/// to opt out — same pattern as the telemetry default-on disclosure.
fn run_mcp_register_phase(config: &Config, dry_run: bool) {
    // Locally-owned config when we synthesize the default — keeps the
    // `&McpRegisterConfig` borrow lifetime simple in both branches.
    let synthesized: Option<crate::mcp_register::McpRegisterConfig> = if config
        .mcp_register
        .is_none()
        && should_auto_register(dry_run)
    {
        let detected = crate::mcp_register::auto_detect_agents();
        if detected.is_empty() {
            None
        } else {
            let agents_label = detected
                .iter()
                .map(|a| a.slug())
                .collect::<Vec<_>>()
                .join(", ");
            if crate::console::is_enabled() {
                eprintln!(
                    "\nNote: registering Jarvy MCP server with detected AI agents: {agents_label}.\n      Disable: set JARVY_MCP_REGISTER=0, or add `[mcp_register] agents = []` to jarvy.toml.\n      Details: https://jarvy.dev/mcp-registration/"
                );
            }
            crate::telemetry::mcp_register_auto_detected(&detected);
            Some(crate::mcp_register::synthesize_auto_register(detected))
        }
    } else {
        None
    };

    let mcp_cfg = match (config.mcp_register.as_ref(), synthesized.as_ref()) {
        (Some(cfg), _) => cfg,
        (None, Some(cfg)) => cfg,
        (None, None) => return,
    };
    if mcp_cfg.is_empty() {
        return;
    }
    let agent_count = mcp_cfg.unique_agents().len();
    let servers_count = mcp_cfg.servers.len() + 1; // +1 for the built-in jarvy entry
    let scope_label = match mcp_cfg.scope {
        crate::mcp_register::McpRegistrationScope::User => "user",
        crate::mcp_register::McpRegistrationScope::Project => "project",
    };

    let _span = tracing::info_span!(
        "mcp_register",
        agents = %agent_count,
        scope = %scope_label,
        dry_run = %dry_run,
    )
    .entered();

    if dry_run {
        println!("\n=== MCP Registration (dry-run) ===");
        println!(
            "[DRY-RUN] Would register {} server(s) with: {:?}",
            servers_count,
            mcp_cfg.unique_agents()
        );
        return;
    }

    chatter!("\n=== MCP Registration ===");
    let started = std::time::Instant::now();
    crate::telemetry::mcp_register_phase_started(agent_count, servers_count, scope_label);

    match crate::mcp_register::apply(mcp_cfg) {
        Ok(report) => {
            chatter!(
                "  Registered {} server(s) across {} agent(s)",
                report.total_applied(),
                report.successes.len()
            );
            for o in &report.successes {
                chatter!("    {:<13} {}", o.agent, o.path.display());
                for w in &o.warnings {
                    chatter!("      warning: {w}");
                }
                crate::telemetry::mcp_register_agent_applied(o.agent, o.applied, &o.path);
            }
            for (target, e) in &report.failures {
                eprintln!(
                    "    {:<13} FAILED ({}): {} — other agents still applied",
                    target.slug(),
                    e.kind(),
                    e
                );
                crate::telemetry::mcp_register_agent_failed(target.slug(), e.kind());
            }
            if !report.refused_custom.is_empty() {
                chatter!(
                    "  Refused {} custom server(s) (set allow_custom_servers = true to apply)",
                    report.refused_custom.len()
                );
            }
            if !report.remote_refused.is_empty() {
                chatter!(
                    "  Refused {} custom server(s) from remote-fetched config (trust boundary)",
                    report.remote_refused.len()
                );
            }
            crate::telemetry::mcp_register_phase_completed(
                report.total_applied(),
                report.agents_touched(),
                report.refused_custom.len(),
                report.remote_refused.len(),
                report.failures.len(),
                started.elapsed(),
            );
        }
        Err(e) => {
            eprintln!("  Warning: MCP registration failed: {e}");
            crate::telemetry::mcp_register_agent_failed("global", e.kind());
            crate::telemetry::mcp_register_phase_completed(0, 0, 0, 0, 1, started.elapsed());
        }
    }
}

/// Decide whether to auto-register the Jarvy MCP server when the
/// project's `jarvy.toml` has no `[mcp_register]` block. Skip in
/// every "this isn't a developer doing setup" context: dry-run,
/// `JARVY_TEST_MODE=1` (integration tests), `cfg(test)` (unit tests
/// that drive the function directly), `JARVY_MCP_REGISTER=0` (user
/// override), and seamless / auto-detected sandboxes (Codespaces,
/// Claude Code, devcontainers — multi-tenant base images shouldn't
/// silently write to `~/.cursor` etc.). Forced sandbox
/// (`JARVY_SANDBOX=1` without auto-detection) is intentionally NOT
/// in this gate so users on a real machine who set the env var for
/// a single command don't get their MCP config silently changed,
/// but a hostile dotfile-driven `JARVY_SANDBOX=1` also doesn't get
/// to suppress an opt-out the user actually wanted — same
/// `is_seamless_auto`-only posture as the telemetry CI auto-disable.
fn should_auto_register(dry_run: bool) -> bool {
    if dry_run {
        return false;
    }
    if cfg!(test) {
        return false;
    }
    if std::env::var("JARVY_TEST_MODE").as_deref() == Ok("1") {
        return false;
    }
    if std::env::var("JARVY_MCP_REGISTER").as_deref() == Ok("0") {
        return false;
    }
    if crate::sandbox::is_seamless_auto() {
        return false;
    }
    true
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
        chatter!("\nAuto-starting {backend} services...");
        match backend_impl.start(&config_path, true) {
            Ok(result) => chatter!("{}", result.message),
            Err(e) => {
                // Services auto-start is advisory — don't fail the setup.
                eprintln!("Warning: Failed to auto-start services: {e}");
            }
        }
    }
}

/// Compare the live agent profile against `[agents.profiles] prefer`
/// and print a hint when they diverge (PRD-058).
///
/// Advisory by design and it never auto-switches: switching a profile
/// swaps the credentials an agent authenticates with, and credentials
/// don't move without an explicit user command. `strict = true` only
/// raises the volume from a stdout hint to a stderr warning — it still
/// does not fail setup, because a wrong-profile machine is a
/// misconfiguration to tell the user about, not a reason to refuse to
/// install their tools.
///
/// No trust gate needed: `[agents.profiles]` is stripped wholesale from
/// remote configs in `Config::mark_remote`, so `prefer` is by
/// construction local.
fn run_agent_profile_hint_phase(config: &Config) {
    let Some(prefer) = config
        .agents
        .as_ref()
        .and_then(|a| a.profiles.as_ref())
        .and_then(|p| p.prefer.as_deref())
        .filter(|p| !p.trim().is_empty())
    else {
        return;
    };
    let strict = config
        .agents
        .as_ref()
        .and_then(|a| a.profiles.as_ref())
        .and_then(|p| p.strict)
        .unwrap_or(false);

    let Ok(check) = crate::agent_profiles::check_preference(prefer) else {
        return; // No store yet, unreadable registry — nothing to compare.
    };
    let matched = check.matched();

    if !matched {
        let agents = check.mismatched.join(", ");
        if strict {
            eprintln!(
                "Warning: this repo prefers agent profile '{prefer}', but {agents} \
                 {} on a different one.\n  Switch with: jarvy agents profile use {prefer}",
                if check.mismatched.len() == 1 {
                    "is"
                } else {
                    "are"
                }
            );
        } else {
            chatter!(
                "\nAgent profile: this repo prefers '{prefer}' ({agents} on another). \
                 Switch with `jarvy agents profile use {prefer}`."
            );
        }
    }

    if crate::observability::telemetry_gate::is_enabled() {
        // Profile names never reach telemetry — `matched` is the whole
        // signal, and the agent slugs are a bounded set.
        tracing::info!(
            event = "agent_profile.setup_hint",
            strict,
            matched,
            mismatched_count = check.mismatched.len(),
            unmanaged_count = check.unmanaged.len(),
        );
    }
}

/// Apply the `[dotfiles]` phase (PRD follow-up for cross-machine
/// dotfile sync). Advisory — never fails `jarvy setup`; on trust-gate
/// refusal or manager failure the setup lead prints a warning and
/// keeps going. Skipped silently when the block is absent.
fn run_dotfiles_phase(config: &Config, dry_run: bool) {
    let Some(cfg) = config.dotfiles.as_ref() else {
        // Emit `dotfiles.phase_absent` so the adoption-rate query
        // has a denominator — otherwise the numerator (phase_started
        // et al.) is unbounded and PMs can't compute "of eligible
        // setup runs, how many opted into dotfiles?"
        crate::dotfiles::emit_phase_absent();
        return;
    };
    if dry_run {
        println!(
            "\n[DRY-RUN] Would apply [dotfiles] via {}",
            cfg.manager.cli()
        );
    } else {
        println!("\nApplying [dotfiles] via {}...", cfg.manager.cli());
    }
    match crate::dotfiles::run_phase(cfg, dry_run) {
        crate::dotfiles::PhaseOutcome::Applied { .. } => {
            println!("Dotfiles applied.");
        }
        crate::dotfiles::PhaseOutcome::NoOp => {
            println!("Dotfiles up to date.");
        }
        crate::dotfiles::PhaseOutcome::Skipped(reason) => {
            use crate::dotfiles::SkipReason;
            println!("Dotfiles skipped: {}", reason.telemetry());
            match reason {
                SkipReason::StowManual => {
                    println!(
                        "  stow requires per-package invocation — jarvy installs it \
                         but does not auto-apply. Run `stow <package>` from your \
                         dotfiles repo."
                    );
                }
                SkipReason::ManagerNotInstalled => {
                    println!(
                        "  Add `{} = \"latest\"` under [provisioner] so jarvy \
                         installs it before this phase runs.",
                        cfg.manager.cli()
                    );
                }
                SkipReason::DryRun | SkipReason::EmptyRepo => {}
            }
        }
        crate::dotfiles::PhaseOutcome::Refused { reason } => {
            let hint = match reason {
                "invalid_repo" => {
                    "The `repo` value starts with `-` or contains a NUL byte, \
                     which would be interpreted as an option by git/chezmoi \
                     (CVE-2017-1000117 class). Set a plain repo URL/shorthand."
                }
                _ => {
                    "Add `allow_remote = true` in the SOURCE config if this is \
                     intentional."
                }
            };
            eprintln!("Warning: [dotfiles] refused ({reason}). {hint}");
        }
        crate::dotfiles::PhaseOutcome::Failed { error, .. } => {
            eprintln!("Warning: [dotfiles] failed: {error}");
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
    // Feed the iterator directly — the previous throwaway Vec was
    // one heap allocation + N pointer-pair writes per setup for no
    // downstream reader (perf F2).
    capture_drift_baseline_borrowed(
        project_dir,
        config_path,
        known_tools.iter().map(|(k, v)| (k, v)),
        track_files,
        auto,
    )
}

/// Borrow-based variant of `capture_drift_baseline` — lets the
/// verify-only auto-baseline path filter `tool_configs` without
/// deep-cloning every entry. Same on-disk output shape.
fn capture_drift_baseline_borrowed<'a>(
    project_dir: &std::path::Path,
    config_path: &std::path::Path,
    known_tools: impl IntoIterator<Item = (&'a String, &'a crate::config::Tool)>,
    track_files: &[String],
    auto: bool,
) {
    let mut state = crate::drift::EnvironmentState::new();
    for (tool_name, tool) in known_tools {
        let binary = crate::tools::spec::binary_for(tool_name);
        if let Ok(path) = which::which(binary) {
            // Probe the actual installed version so `drift check`
            // compares like-for-like. Storing the config constraint
            // (`"latest"`, `"^20"`) here caused every check to
            // false-positive because VersionPolicy::versions_match
            // falls back to exact-string compare for non-semver
            // values (Codex P1 / parallel-review item 1).
            let installed = crate::drift::detector::get_tool_version(binary);
            state.set_tool_with_installed(
                tool_name,
                &tool.version,
                installed.as_deref(),
                &path,
                &detect_install_method(tool_name),
            );
        }
    }
    for file_path in track_files {
        // Reject entries whose path shape could escape project_dir —
        // absolute paths (Path::join drops the base) or any parent-dir
        // (`..`) component. Same primitive a hostile remote config
        // would abuse via `track_files = ["/etc/shadow"]` (security F1).
        if !crate::drift::track_file_is_safe(file_path) {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "drift.track_files.refused",
                    reason = "unsafe_path_shape",
                );
            }
            continue;
        }
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
        Err(e) => {
            eprintln!("Warning: Could not save drift detection state: {}", e);
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "drift.state.io_failed",
                    op = "save",
                    error_kind = e.kind_label(),
                    error = %e,
                );
            }
        }
        Ok(()) => {
            // Single event with bounded `source` discriminant so
            // downstream funnel queries don't have to UNION two
            // event names, and any future field lands in one place
            // (parallel-review item 13 / obs F5). Baseline captures
            // that lose tools to the binary_for lookup are still
            // implicit-shape-visible via tool_count vs known_tools
            // count.
            let source = if auto { "setup_seamless" } else { "setup" };
            let provider = if auto {
                crate::sandbox::detect()
                    .map(|e| e.provider.to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            };
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::info!(
                    event = "drift.baseline.captured",
                    tool_count = state.tool_count(),
                    source = source,
                    provider = %provider,
                );
            }
            if auto {
                eprintln!(
                    "[jarvy] auto-baselined drift state for seamless mode ({} tools)",
                    state.tool_count()
                );
            } else {
                chatter!(
                    "\nDrift detection baseline captured ({} tools)",
                    state.tool_count()
                );
            }
        }
    }
}

/// Resolve the effective jarvy.toml path for a command, applying
/// PRD-047 workspace auto-context. If cwd sits inside a declared
/// workspace member, returns that member's jarvy.toml (synthesizing
/// from workspace defaults when the member has no per-member config).
/// Otherwise returns `file` verbatim.
///
/// Centralized so the `auto_detect_project → resolve_workspace_project →
/// fallback` glue lives in one place instead of being re-inlined per
/// command (was: setup arm + doctor + drift + context).
pub(crate) fn effective_config_path(file: &str) -> std::path::PathBuf {
    auto_detect_project(file)
        .and_then(|m| resolve_workspace_project(file, &m).ok())
        .unwrap_or_else(|| std::path::PathBuf::from(file))
}

/// Auto-context detection (PRD-047 phase 2). When `jarvy setup` is
/// invoked without `--project` AND cwd sits inside a declared
/// workspace member, return that member's name so the caller can
/// scope setup automatically. Returns `None` when:
/// - the file argument's parent already IS the workspace root, OR
/// - no `[workspace]` section is found walking up, OR
/// - cwd doesn't sit inside any declared member.
pub(crate) fn auto_detect_project(file: &str) -> Option<String> {
    let project_dir = crate::paths::config_parent_dir(file);
    let ctx = crate::workspace::find_workspace_root(&project_dir)?;
    let root_dir = ctx.root_config.parent()?;
    // Only return Some(member) when the supplied `file` actually
    // sits BELOW the workspace root — otherwise the user is already
    // at the root and explicit `--project` is required.
    let canonical_dir = project_dir.canonicalize().ok()?;
    let canonical_root = root_dir.canonicalize().ok()?;
    if canonical_dir == canonical_root {
        return None;
    }
    ctx.current_member
}

/// Workspace-aware project resolution for `jarvy setup --project <name>`
/// (PRD-047 phase 2).
///
/// Returns the path setup should read instead of the supplied `file`:
///
/// - If `name == "current"`: walk up from `cwd` to find a workspace
///   root, then resolve the member that contains cwd. Errors when not
///   inside any member.
/// - If `name` matches an exact / glob-expanded member: return that
///   member's `jarvy.toml` path.
/// - If the member has no `jarvy.toml` of its own (workspace-defaults
///   case): synthesize the merged root-only config into a tempfile and
///   return that path. The tempfile lives until process exit — fine
///   for setup which runs once and exits.
/// - On any failure (no workspace, unknown member, traversal): return
///   a structured error so dispatch can produce a clean diagnostic.
pub(crate) fn resolve_workspace_project(
    root_file: &str,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let project_dir = crate::paths::config_parent_dir(root_file);

    let ctx = crate::workspace::find_workspace_root(&project_dir).ok_or_else(|| {
        format!(
            "no [workspace] section found walking up from {}",
            project_dir.display()
        )
    })?;
    let workspace_root = ctx
        .root_config
        .parent()
        .ok_or_else(|| "workspace root config has no parent".to_string())?
        .to_path_buf();
    let resolved = ctx.workspace.resolved_members(&workspace_root);

    let target_member: String = if name == "current" {
        let cwd = std::env::current_dir().map_err(|e| format!("cannot read current dir: {e}"))?;
        ctx.current_member.clone().ok_or_else(|| {
            format!(
                "cwd {} is not inside any declared workspace member",
                cwd.display()
            )
        })?
    } else {
        resolved
            .into_iter()
            .find(|m| m == name)
            .ok_or_else(|| format!("`{name}` is not a declared workspace member"))?
    };

    // Containment + path-traversal refusal (P0 #3 in the parallel
    // review applies here too).
    let candidate = std::path::Path::new(&target_member);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!(
            "member `{target_member}` would escape the workspace root"
        ));
    }
    let member_dir = workspace_root.join(&target_member);
    let member_toml = member_dir.join("jarvy.toml");

    if member_toml.exists() {
        return Ok(member_toml);
    }

    // No per-member jarvy.toml. Synthesize the merged config and
    // stash it in a content-addressed cache under ~/.jarvy/cache/
    // synthesized/ instead of /tmp. Previously this called
    // `NamedTempFile::keep()` which discards the auto-delete handler;
    // every workspace-defaults setup run permanently leaked a file to
    // /tmp (review item 7 / Sec F3). The cache path is hashed by
    // (workspace root + member name) so re-runs reuse the same file
    // and concurrent processes never collide.
    let raw = std::fs::read_to_string(&ctx.root_config)
        .map_err(|e| format!("read {}: {e}", ctx.root_config.display()))?;
    let root_value: toml::Value =
        toml::from_str(&raw).map_err(|e| format!("parse {}: {e}", ctx.root_config.display()))?;
    let merged = crate::workspace::merge_configs(
        &root_value,
        &toml::Value::Table(toml::Table::new()),
        &ctx.workspace.effective_inherit(),
    );
    let serialized =
        toml::to_string_pretty(&merged).map_err(|e| format!("serialize merged config: {e}"))?;

    let path = synthesized_cache_path(&workspace_root, &target_member)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, &serialized).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

/// Stable cache path for a synthesized member config. Lives under
/// `~/.jarvy/cache/synthesized/` so it survives the process (the
/// downstream setup phases read it via path, not handle) without
/// leaking into the global /tmp namespace. Hash key = canonical
/// workspace root + member name; the file is overwritten on every
/// run, which keeps it in sync with edits to the root config.
fn synthesized_cache_path(
    workspace_root: &std::path::Path,
    member: &str,
) -> Result<std::path::PathBuf, String> {
    use sha2::{Digest, Sha256};
    let canonical = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    hasher.update(b"\0");
    hasher.update(member.as_bytes());
    let digest = hasher.finalize();
    let hex = hex::encode(&digest[..12]);
    let base = dirs::home_dir()
        .ok_or_else(|| "no home directory; cannot stage synthesized config".to_string())?
        .join(".jarvy")
        .join("cache")
        .join("synthesized");
    Ok(base.join(format!("{hex}.toml")))
}

/// True iff `dir` is the synthesized-config cache root used by
/// `resolve_workspace_project` when a member has no per-member
/// `jarvy.toml`. The continuous-discovery phase reads this to scan
/// the real project tree instead of `~/.jarvy/cache/synthesized/`.
///
/// Inner helper `is_synthesized_cache_dir_in` takes an explicit
/// `cache_root` so unit tests can pass a fake root without polluting
/// the user's real `~/.jarvy/`. The public wrapper resolves the real
/// cache root and delegates.
fn is_synthesized_cache_dir(dir: &std::path::Path) -> bool {
    let Some(cache_root) = synthesized_cache_root() else {
        return false;
    };
    is_synthesized_cache_dir_in(dir, &cache_root)
}

fn is_synthesized_cache_dir_in(dir: &std::path::Path, cache_root: &std::path::Path) -> bool {
    // Compare canonical paths so a symlink-into-cache or
    // `~/.jarvy/cache/synthesized/../synthesized/` both resolve
    // correctly. Returning `false` on canonicalize failure is the
    // safe default: we'd rather skip the scan-root redirect (and
    // pick up no marker files) than redirect to cwd erroneously.
    let canon_dir = match dir.canonicalize() {
        Ok(d) => d,
        Err(e) => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::debug!(
                    event = "discover.tempfile_detect.canonicalize_failed",
                    target = "project_dir",
                    path = %dir.display(),
                    error = %e,
                );
            }
            return false;
        }
    };
    let canon_root = match cache_root.canonicalize() {
        Ok(r) => r,
        Err(_) => {
            // Cache root doesn't exist yet — common case (first run).
            // The dir under test can't possibly be inside it.
            return false;
        }
    };
    canon_dir.starts_with(&canon_root)
}

fn synthesized_cache_root() -> Option<std::path::PathBuf> {
    Some(
        dirs::home_dir()?
            .join(".jarvy")
            .join("cache")
            .join("synthesized"),
    )
}

/// Continuous discovery (PRD-044 phase 2). After `jarvy setup`
/// finishes its install phases we run `discover::analyze` and warn
/// when project marker files imply tools that aren't pinned in
/// `[provisioner]`. NEVER mutates jarvy.toml — that requires an
/// explicit `jarvy discover --apply`. Emits structured telemetry so
/// dashboards can graph "setup runs that hint at missing tools."
///
/// Quiet by default when:
/// - `JARVY_TEST_MODE=1` (test runs)
/// - The setup ran in dry-run mode (we don't want to nudge during a
///   preview)
/// - There are no new suggestions
fn run_continuous_discover_phase(file: &str) {
    if std::env::var("JARVY_TEST_MODE").as_deref() == Ok("1") {
        return;
    }

    // Pick the directory to SCAN for marker files. Normally this is
    // the parent of `file` — but when `file` is a synthesized
    // member-config from `setup --project <name>` (path under
    // `~/.jarvy/cache/synthesized/`), the parent isn't the project
    // tree. In that case, fall back to cwd, which is where the user
    // actually launched `jarvy setup`.
    let raw_parent = crate::paths::config_parent_dir(file);
    let scan_root_redirected = is_synthesized_cache_dir(&raw_parent);
    let project_dir = if scan_root_redirected {
        let cwd = std::env::current_dir().unwrap_or_else(|_| raw_parent.clone());
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::debug!(
                event = "discover.setup_advisory.scan_root_redirected",
                from = %raw_parent.display(),
                to = %cwd.display(),
                reason = "synthesized_member_config",
            );
        }
        cwd
    } else {
        raw_parent
    };

    let existing_text = std::fs::read_to_string(file).ok();
    let already_configured: std::collections::HashSet<String> = existing_text
        .as_deref()
        .and_then(|t| t.parse::<toml::Table>().ok())
        .and_then(|t| t.get("provisioner").and_then(|v| v.as_table()).cloned())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    let known: std::collections::HashSet<String> = crate::tools::registry::registered_tool_names()
        .into_iter()
        .collect();

    let report = crate::discover::analyze(&project_dir, &already_configured, &known);
    let new_count = report.required.len();
    if new_count == 0 {
        return;
    }

    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "discover.setup_advisory",
            new_tools = new_count,
            uninstallable = report.uninstallable.len(),
            scan_root = %project_dir.display(),
            scan_root_redirected,
        );
    }

    chatter!("");
    chatter!(
        "Tip: `jarvy discover` found {new_count} additional tool(s) implied by your project files \
         that aren't yet in [provisioner]:"
    );
    for tool in &report.required {
        chatter!("  - {} ({})", tool.name, tool.reason);
    }
    chatter!("Run `jarvy discover --apply` to pin them.");
}

/// State the setup path exposes back to the outer `run_setup`
/// caller so the `setup.completed` event can attribute what
/// happened on the maintenance phase without every skip site
/// needing its own emission. Bounded label — see analytics F2 in
/// the PRD-057 review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MaintenanceState {
    /// Cache read + summary evaluated (whether or not the cache
    /// had entries).
    Ran,
    /// `[maintenance] check_updates = false`.
    SkippedConfig,
    /// `JARVY_CHECK_UPDATES=0` env kill-switch.
    SkippedEnv,
    /// Sandbox detected via `crate::sandbox`.
    SkippedSandbox,
    /// CI detected via `crate::ci`.
    SkippedCi,
    /// Non-TTY (piped setup, npm predev) — cache-read still ran,
    /// but the background refresher spawn is suppressed.
    SkippedNonTty,
    /// Remote-fetched config attempted to enable check_updates
    /// without `[maintenance] allow_remote = true`.
    RemoteRefused,
    /// Ran, but the cache had no entries yet (first-ever setup).
    EmptyCache,
}

impl MaintenanceState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ran => "ran",
            Self::SkippedConfig => "skipped_config",
            Self::SkippedEnv => "skipped_env",
            Self::SkippedSandbox => "skipped_sandbox",
            Self::SkippedCi => "skipped_ci",
            Self::SkippedNonTty => "skipped_non_tty",
            Self::RemoteRefused => "remote_refused",
            Self::EmptyCache => "empty_cache",
        }
    }
}

/// Freshness advisory phase (PRD-057). Reads the on-disk cache
/// (never network) to print a one-line summary of tools with newer
/// upstream versions, then spawns a detached background process to
/// refresh the cache for the *next* invocation. Never blocks setup
/// completion — cache-hit reads are sub-millisecond and the spawn
/// returns as soon as the child is fork/exec'd.
///
/// Skip conditions (in order):
/// - `[maintenance] check_updates = false` or the
///   `JARVY_CHECK_UPDATES=0` kill-switch.
/// - `[maintenance] notify_on = "never"` — silences the summary
///   but still spawns the refresher (adoption signal stays honest).
/// - Sandbox / CI / non-TTY invocations skip the *spawn* only;
///   cache reads still fire so a scheduled CI job can populate
///   them via `jarvy check-updates`.
fn run_maintenance_phase(config: &Config, file: &str) -> MaintenanceState {
    let maintenance = config.maintenance.clone().unwrap_or_default();
    let maintenance_started = std::time::Instant::now();

    if let Err(reason) = crate::maintenance::should_run(&maintenance) {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!(
                event = "maintenance.phase_skipped",
                reason = reason.as_str(),
            );
        }
        return match reason {
            crate::maintenance::SkipReason::DisabledByConfig => MaintenanceState::SkippedConfig,
            crate::maintenance::SkipReason::DisabledByEnv => MaintenanceState::SkippedEnv,
            crate::maintenance::SkipReason::Sandbox => MaintenanceState::SkippedSandbox,
            crate::maintenance::SkipReason::Ci => MaintenanceState::SkippedCi,
            // The other SkipReason variants aren't returned by
            // `should_run` today; keep them mapped for forward
            // compatibility.
            crate::maintenance::SkipReason::NonTty => MaintenanceState::SkippedNonTty,
            crate::maintenance::SkipReason::RemoteRefused => MaintenanceState::RemoteRefused,
            _ => MaintenanceState::SkippedConfig,
        };
    }

    if maintenance.remote_refused() {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "maintenance.remote_refused",
                reason = "allow_remote_not_set",
            );
        }
        return MaintenanceState::RemoteRefused;
    }

    // 1. Cache-read summary line.
    let cache = crate::maintenance::cache::load().unwrap_or_default();
    let (report, ready_for_summary) = build_summary_from_cache(&cache, config, &maintenance);
    if ready_for_summary
        && matches!(
            maintenance.notify_on,
            crate::maintenance::config::NotifyOn::Setup
        )
        && let Some(line) = crate::maintenance::reporter::setup_summary_line(&report)
    {
        println!("\n{}", line);
    }
    if crate::observability::telemetry_gate::is_enabled() {
        for check in &report.updates {
            // Reshaped stale_tool event — see check_updates_cmd.rs
            // for the review-item rationale. `mode = "setup_read"`
            // distinguishes cache-read emissions from the refresher's
            // `cli_foreground` / `cli_background` runs.
            let lag = crate::maintenance::checker::LagBucket::classify(
                check.installed.as_deref(),
                check.latest.as_deref(),
                check.direction,
            );
            tracing::debug!(
                event = "maintenance.stale_tool",
                tool = %check.tool,
                tool_kind = crate::maintenance::checker::tool_kind_for_backend(&check.backend),
                backend = %check.backend,
                direction = check.direction.as_str(),
                lag_bucket = lag.as_str(),
                from_cache = check.from_cache,
                mode = "setup_read",
            );
        }
        tracing::info!(
            event = "maintenance.phase_completed",
            mode = "setup_read",
            tools_checked = report.summary.tools_checked,
            updates_available = report.summary.updates_available,
            unchecked = report.summary.unchecked,
            errors = report.summary.errors,
            duration_ms = maintenance_started.elapsed().as_millis() as u64,
        );
    }
    crate::telemetry::maintenance_stale_tools(report.summary.updates_available as u64, None);

    // Preserve the "did the cache actually have entries?" answer
    // for the return value so the outer `setup.completed` event
    // can distinguish first-ever setup (empty cache) from
    // steady-state adoption.
    let ran_state = if ready_for_summary {
        MaintenanceState::Ran
    } else {
        MaintenanceState::EmptyCache
    };

    // 2. Background refresher — TTY-only. Non-TTY setups (piped
    // CI, npm predev) don't need the summary next run and the
    // detached spawn is friction under those runners.
    let is_tty = atty_stderr();
    let is_sandbox = crate::sandbox::is_sandbox();
    let is_ci = crate::ci::is_ci();
    if !is_tty || is_sandbox || is_ci {
        // Split what used to be a compound `sandbox_or_ci` label
        // into distinct bounded reasons — see the PRD-057
        // observability F7 + analytics F8 review items.
        let (reason, phase_state) = if !is_tty {
            ("non_tty", MaintenanceState::SkippedNonTty)
        } else if is_sandbox {
            ("sandbox", MaintenanceState::SkippedSandbox)
        } else {
            ("ci", MaintenanceState::SkippedCi)
        };
        if crate::observability::telemetry_gate::is_enabled() {
            // Promoted from debug! to info! so the "why isn't my
            // cache warming?" signal survives default log filters
            // (obs F7). Volume is bounded — once per setup call.
            tracing::info!(
                event = "maintenance.phase_skipped",
                reason = reason,
                stage = "background_spawn",
                duration_ms = maintenance_started.elapsed().as_millis() as u64,
            );
        }
        // The cache-read completed successfully; only the
        // background spawn was suppressed. Return `Ran` /
        // `EmptyCache` when the summary path itself succeeded so
        // the outer telemetry doesn't confuse "we couldn't warm
        // for next time" with "we never ran at all". But when
        // the trigger for the skip is a signal the outer caller
        // needs to attribute (non_tty on a headless CI runner),
        // surface it explicitly.
        return if !is_tty {
            MaintenanceState::SkippedNonTty
        } else {
            phase_state
        };
    }

    match spawn_background_refresher(file) {
        Ok(pid) => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::info!(event = "maintenance.background_spawned", pid = pid,);
            }
        }
        Err(kind) => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "maintenance.background_spawn_failed",
                    error_kind = kind,
                );
            }
        }
    }

    ran_state
}

/// Assemble a `Report` from cache entries only — no backend
/// probes. Returns the report + a flag indicating whether the
/// cache actually had data (used to skip the summary line on
/// first-ever setup runs).
///
/// PRD-057 review items:
/// - perf F1: routes through `resolver::resolve_targets_cheap` so
///   the setup summary path no longer spawns `<cmd> --version` × N
///   subprocesses.
/// - maint F2: reuses `checker::run_check_cache_only` for the
///   classify/sort/summary logic that used to be re-implemented
///   inline here.
fn build_summary_from_cache(
    cache: &crate::maintenance::cache::CacheStore,
    config: &Config,
    maintenance: &crate::maintenance::MaintenanceConfig,
) -> (crate::maintenance::checker::Report, bool) {
    use crate::maintenance::cache::CacheStore;
    use crate::maintenance::checker::run_check_cache_only;
    use crate::maintenance::resolver::{ResolveInput, resolve_targets_cheap};

    // Setup path deliberately skips `cargo install --list`
    // detection — the fallback `<name> --version` is fine for
    // summary-line rendering, and the background refresher fills
    // in accurate entries via the CLI path.
    let input = ResolveInput {
        provisioner_tools: config.get_tool_configs().into_keys().collect(),
        cargo_packages: config
            .cargo
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        npm_packages: config
            .npm
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        pip_packages: config
            .pip
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        gem_packages: config
            .gem
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        go_packages: config
            .go
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        nuget_packages: config
            .nuget
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        cargo_installed: Default::default(),
    };
    let (targets, unchecked) = resolve_targets_cheap(&input, maintenance);

    // Compute `had_any_entry` before handing targets off to the
    // shared checker — once we own the report the answer is a
    // simple `tools_checked > 0`, but we want the exact
    // "any cache entry present at all?" semantic including
    // error entries.
    let had_any_entry = targets.iter().any(|t| {
        cache
            .entries
            .contains_key(&CacheStore::key(t.backend.name(), &t.pkg_id))
    });

    let report = run_check_cache_only(targets, cache, unchecked);
    (report, had_any_entry)
}

fn atty_stderr() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}

/// Windows creation-flag constants — inspected by the QA test
/// suite (see PRD-057 review item qa F2) so a typo in the
/// spawn-side code path is caught even on CI runners that only
/// exercise the Unix branch.
#[cfg(windows)]
pub(crate) const REFRESHER_DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
pub(crate) const REFRESHER_CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Configure a `Command` for the detached background refresher.
/// Extracted from [`spawn_background_refresher`] so tests can
/// exercise the argv / stdio / OS-specific flag wiring without
/// actually forking a child (which is racy under `cargo test` and
/// impossible under `#[cfg(windows)]` on macOS/Linux CI).
pub(crate) fn configure_child(cmd: &mut std::process::Command, file: &str) {
    cmd.arg("check-updates")
        .arg("--refresh")
        .arg("--background")
        .arg("--file")
        .arg(file)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own process group — parent can exit without waiting and
        // the child survives terminal hang-up.
        cmd.process_group(0);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NO_WINDOW — no inherited console,
        // no popup, no wait.
        cmd.creation_flags(REFRESHER_DETACHED_PROCESS | REFRESHER_CREATE_NO_WINDOW);
    }
}

/// Spawn a detached `jarvy check-updates --refresh --background`
/// child. Returns the child PID on success, a bounded telemetry
/// label on failure. Never blocks — the child inherits an
/// empty stdio and its own process group (Unix) / detached
/// console (Windows) so terminal exit doesn't SIGHUP it.
fn spawn_background_refresher(file: &str) -> Result<u32, &'static str> {
    let exe = std::env::current_exe().map_err(|_| "current_exe")?;
    let mut cmd = std::process::Command::new(exe);
    configure_child(&mut cmd, file);
    match cmd.spawn() {
        Ok(child) => Ok(child.id()),
        Err(e) => Err(match e.kind() {
            std::io::ErrorKind::NotFound => "exec",
            std::io::ErrorKind::PermissionDenied => "permission_denied",
            _ => "io",
        }),
    }
}

/// Extracted from the seamless-mode branch of `run_setup` so the
/// 5-conjunct eligibility rule is testable without mocking
/// `sandbox::is_seamless` / `paths::state_json`.
fn auto_baseline_eligible(
    drift_enabled: bool,
    seamless: bool,
    needs_install_empty: bool,
    unknown_empty: bool,
    state_json_exists: bool,
) -> bool {
    !drift_enabled && seamless && needs_install_empty && unknown_empty && !state_json_exists
}

#[cfg(test)]
mod tests {
    //! Smoke tests for the extracted phase helpers (review item 21 / item 8
    //! follow-up). These run the dry-run path of each phase so we catch
    //! signature drift and panics — not the full install behavior.

    use super::*;

    // -------------------------------------------------------------
    // `is_synthesized_cache_dir_in` — pure-function variant of the
    // tempfile-detection predicate. Uses an injected `cache_root` so
    // tests don't pollute `~/.jarvy/cache/synthesized/` (QA F4/F8 +
    // Maint F6 — keep `unwrap_or(false)` semantics, DI the cache root).
    // -------------------------------------------------------------

    #[test]
    fn is_synthesized_cache_dir_matches_dir_under_cache_root() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_root = tmp.path().join("cache").join("synthesized");
        std::fs::create_dir_all(&cache_root).unwrap();
        // A file laid down inside the cache root canonicalizes under it.
        let probe = cache_root.join("abc123.toml");
        std::fs::write(&probe, "").unwrap();
        // Function tests the PARENT directory of the probe file,
        // which is the cache root itself.
        assert!(is_synthesized_cache_dir_in(&cache_root, &cache_root));
    }

    #[test]
    fn is_synthesized_cache_dir_rejects_unrelated_path() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_root = tmp.path().join("cache").join("synthesized");
        std::fs::create_dir_all(&cache_root).unwrap();
        let unrelated = tmp.path().join("project");
        std::fs::create_dir_all(&unrelated).unwrap();
        assert!(!is_synthesized_cache_dir_in(&unrelated, &cache_root));
    }

    #[test]
    fn is_synthesized_cache_dir_rejects_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_root = tmp.path().join("cache").join("synthesized");
        std::fs::create_dir_all(&cache_root).unwrap();
        let missing = tmp.path().join("does-not-exist");
        // canonicalize fails → false (safe default — don't redirect
        // scan root when we can't be sure).
        assert!(!is_synthesized_cache_dir_in(&missing, &cache_root));
    }

    #[test]
    fn is_synthesized_cache_dir_rejects_when_cache_root_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let probe = tmp.path().join("project");
        std::fs::create_dir_all(&probe).unwrap();
        let missing_cache_root = tmp.path().join("cache").join("synthesized");
        // Cache root doesn't exist (first run) — function returns false
        // so the dir under test is treated as a normal project dir.
        assert!(!is_synthesized_cache_dir_in(&probe, &missing_cache_root));
    }

    // -------------------------------------------------------------
    // `render_package_preview` — pure-function variant of the dry-run
    // preview body. These pin the announcement string verbatim so the
    // documented "stable contract" doesn't drift silently.
    // -------------------------------------------------------------

    #[test]
    fn render_package_preview_zero_count() {
        let out: String = render_package_preview("npm", "project-local", std::iter::empty());
        assert!(
            out.starts_with("[DRY-RUN] Would install 0 packages via `npm install` (project-local)"),
            "got: {out:?}"
        );
        // No package-name lines for empty input.
        assert_eq!(out.lines().count(), 1, "got: {out:?}");
    }

    #[test]
    fn render_package_preview_singular() {
        let out = render_package_preview("cargo", "user-global", ["cargo-watch"]);
        assert!(
            out.contains("Would install 1 package via `cargo install` (user-global)"),
            "expected singular 'package', got: {out:?}"
        );
        assert!(out.contains("[DRY-RUN]   - cargo-watch"));
    }

    #[test]
    fn render_package_preview_plural() {
        let out = render_package_preview("npm", "project-local", ["b", "a", "c"]);
        assert!(out.contains("Would install 3 packages via `npm install` (project-local)"));
        // Sorted output.
        let body: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("[DRY-RUN]   - "))
            .collect();
        assert_eq!(
            body,
            ["[DRY-RUN]   - a", "[DRY-RUN]   - b", "[DRY-RUN]   - c"]
        );
    }

    #[test]
    fn render_package_preview_dotnet_label_uses_canonical_string() {
        // The dotnet branch is what `examples_validation` pins verbatim.
        let out = render_package_preview(
            ".NET global tool",
            "machine-global",
            ["dotnet-ef", "csharpier"],
        );
        assert!(
            out.contains(
                "[DRY-RUN] Would install 2 .NET global tool(s) via `dotnet tool update -g` (machine-global)"
            ),
            "got: {out:?}"
        );
    }

    #[test]
    fn render_package_preview_redacts_control_bytes() {
        // A hostile `[nuget]` key with ESC must not reach the output.
        let out = render_package_preview(".NET global tool", "machine-global", ["\u{1b}[2J"]);
        assert!(
            !out.contains('\u{1b}'),
            "control byte leaked through preview: {out:?}"
        );
        // The redaction yields `?` per control char.
        assert!(out.contains("[DRY-RUN]   - ?"));
    }

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
    fn resolve_git_phase_skips_without_git_block() {
        let cfg = config_from("[provisioner]\ngit = \"latest\"\n");
        assert!(matches!(resolve_git_phase(&cfg), GitPhaseDecision::Skip));
    }

    #[test]
    fn resolve_git_phase_local_config_applies_with_declared_scope() {
        let cfg = config_from("[provisioner]\n\n[git]\nuser_name = \"x\"\nscope = \"global\"\n");
        match resolve_git_phase(&cfg) {
            GitPhaseDecision::Apply { config, is_remote } => {
                assert!(!is_remote);
                assert_eq!(config.scope, crate::git::ConfigScope::Global);
            }
            other => panic!("expected Apply, got {other:?}"),
        }
    }

    #[test]
    fn resolve_git_phase_refuses_remote_without_allow_remote() {
        let mut cfg = config_from("[provisioner]\n\n[git]\nuser_name = \"x\"\n");
        cfg.mark_remote();
        assert!(matches!(resolve_git_phase(&cfg), GitPhaseDecision::Refused));
    }

    #[test]
    fn resolve_git_phase_remote_authorized_forced_to_local() {
        // Even with allow_remote + an explicit global scope, a remote config is
        // clamped to --local so it can never write ~/.gitconfig.
        let mut cfg = config_from(
            "[provisioner]\n\n[git]\nuser_name = \"x\"\nscope = \"global\"\nallow_remote = true\n",
        );
        cfg.mark_remote();
        match resolve_git_phase(&cfg) {
            GitPhaseDecision::Apply { config, is_remote } => {
                assert!(is_remote);
                assert_eq!(
                    config.scope,
                    crate::git::ConfigScope::Local,
                    "remote writes must be forced to --local"
                );
            }
            other => panic!("expected Apply(local), got {other:?}"),
        }
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

    // ---- AI hooks phase coverage -------------------------------------

    #[test]
    fn run_ai_hooks_phase_skips_when_section_missing() {
        // No [ai_hooks] section at all → silent no-op, no panic, no
        // disk writes. dry_run = false to exercise the early return.
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"
            "#,
        );
        run_ai_hooks_phase(&cfg, false);
    }

    #[test]
    fn run_ai_hooks_phase_skips_when_empty() {
        // `agents = []` produces is_empty() == true → also a no-op.
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"

            [ai_hooks]
            agents = []
            "#,
        );
        run_ai_hooks_phase(&cfg, false);
    }

    // ---- MCP register phase coverage --------------------------------

    #[test]
    fn run_mcp_register_phase_skips_when_section_missing() {
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"
            "#,
        );
        run_mcp_register_phase(&cfg, false);
    }

    #[test]
    fn run_mcp_register_phase_skips_when_empty() {
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"

            [mcp_register]
            agents = []
            "#,
        );
        run_mcp_register_phase(&cfg, false);
    }

    #[test]
    fn run_mcp_register_phase_dry_run_does_not_write_disk() {
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"

            [mcp_register]
            agents = ["claude-code"]
            "#,
        );
        run_mcp_register_phase(&cfg, true);
    }

    #[test]
    fn run_ai_hooks_phase_dry_run_does_not_write_disk() {
        // dry_run = true must NOT touch ~/.claude or any agent settings.
        let cfg = config_from(
            r#"
            [provisioner]
            git = "latest"

            [ai_hooks]
            agents = ["claude-code"]

            [[ai_hooks.hook]]
            use = "block-rm-rf"
            "#,
        );
        // We can't easily assert the negative without a HomeGuard here,
        // but the phase must complete without panicking. Coupled with
        // the explicit dry-run integration test in
        // tests/ai_hooks_integration.rs that asserts no settings file
        // is created, the contract is covered.
        run_ai_hooks_phase(&cfg, true);
    }

    // ----- PRD-047 phase 2: --project resolution ----------------------

    #[test]
    fn resolve_workspace_project_returns_member_toml_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("jarvy.toml"),
            r#"
[workspace]
members = ["apps/web"]

[provisioner]
git = "latest"
"#,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
        std::fs::write(
            tmp.path().join("apps/web/jarvy.toml"),
            "[provisioner]\nnode = \"20\"\n",
        )
        .unwrap();
        let root = tmp.path().join("jarvy.toml");
        let resolved = resolve_workspace_project(root.to_str().unwrap(), "apps/web").unwrap();
        assert_eq!(resolved, tmp.path().join("apps/web/jarvy.toml"));
    }

    #[test]
    fn resolve_workspace_project_synthesizes_when_member_has_no_toml() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("jarvy.toml"),
            r#"
[workspace]
members = ["apps/api"]

[provisioner]
git = "latest"
"#,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("apps/api")).unwrap();
        // No apps/api/jarvy.toml — synthesize.
        let root = tmp.path().join("jarvy.toml");
        let resolved = resolve_workspace_project(root.to_str().unwrap(), "apps/api").unwrap();
        assert!(resolved.exists());
        let content = std::fs::read_to_string(&resolved).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert!(
            parsed.get("provisioner").is_some(),
            "merged config must carry inherited provisioner"
        );
    }

    #[test]
    fn resolve_workspace_project_rejects_unknown_name() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("jarvy.toml"),
            "[workspace]\nmembers = [\"apps/web\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
        let root = tmp.path().join("jarvy.toml");
        let err = resolve_workspace_project(root.to_str().unwrap(), "apps/ghost").unwrap_err();
        assert!(err.contains("ghost"), "got: {err}");
    }

    #[test]
    fn resolve_workspace_project_no_workspace_block_errors() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("jarvy.toml"),
            "[provisioner]\ngit = \"latest\"\n",
        )
        .unwrap();
        let root = tmp.path().join("jarvy.toml");
        let err = resolve_workspace_project(root.to_str().unwrap(), "apps/web").unwrap_err();
        assert!(err.contains("workspace"), "got: {err}");
    }

    // -------------------------------------------------------------
    // `auto_baseline_eligible`: six named cases pinning each of the
    // five conjuncts. One TRUE row, five FALSE rows (each flipping a
    // single guard). Chosen over the 32-row exhaustive matrix.
    // -------------------------------------------------------------

    #[test]
    fn all_clean_verify_only_eligible() {
        // The one row that returns TRUE: drift not explicit, in seamless,
        // no installs pending, no unknown tools, no existing state.json.
        assert!(auto_baseline_eligible(false, true, true, true, false));
    }

    #[test]
    fn drift_explicit_ineligible() {
        // If [drift].enabled = true, the explicit path handles baseline;
        // the auto path must stay out of it.
        assert!(!auto_baseline_eligible(true, true, true, true, false));
    }

    #[test]
    fn non_seamless_ineligible() {
        // Auto-baseline is a seamless-mode primitive; on a real host it
        // must not fire.
        assert!(!auto_baseline_eligible(false, false, true, true, false));
    }

    #[test]
    fn has_pending_installs_ineligible() {
        // A partial-match baseline would ship the wrong sentinel; wait
        // until the install queue drains.
        assert!(!auto_baseline_eligible(false, true, false, true, false));
    }

    #[test]
    fn has_unknown_tools_ineligible() {
        // Unregistered tools would silently drop out of the baseline; wait
        // for them to be resolved.
        assert!(!auto_baseline_eligible(false, true, true, false, false));
    }

    #[test]
    fn existing_state_json_ineligible() {
        // Never overwrite a hand-crafted or previously-captured baseline.
        assert!(!auto_baseline_eligible(false, true, true, true, true));
    }
}
