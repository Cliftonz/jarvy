//! Route configured tools to the right freshness backend (PRD-057).
//!
//! Two input surfaces feed the checker:
//!
//! 1. `[provisioner]` entries — CLI tools installed via native
//!    package managers (brew / apt / dnf / winget / …). Routed by
//!    consulting the registered [`ToolSpec`] and picking the
//!    per-OS installer.
//! 2. `[cargo]` / `[npm]` package sections — language-scoped
//!    binaries. Routed straight to the matching backend using the
//!    section's package name as `pkg_id`.
//!
//! Anything the router can't cleanly resolve — version managers,
//! custom-install tools, absent backend, ignored-by-config — lands
//! in the report's `unchecked` bucket with a bounded reason label
//! so telemetry aggregations stay stable.

use std::collections::{HashMap, HashSet};
use std::process::Command;

use crate::packages::common::validate_package_name;
use crate::tools::spec::ToolSpec;

// Backend imports — several are per-OS via `#[cfg(...)]` branches
// in `provisioner_backend`. Rust's dead-code check runs after cfg
// pruning, so the whole list is gated with `allow(unused_imports)`
// rather than a fan-out of per-cfg `use` blocks that would drift.
#[allow(unused_imports)]
use super::backends::{
    FreshnessBackend, apk::ApkBackend, apt::AptBackend, brew::BrewBackend, cargo::CargoBackend,
    choco::ChocoBackend, dnf::DnfBackend, gem::GemBackend, go::GoBackend, npm::NpmBackend,
    nuget::NugetBackend, pacman::PacmanBackend, pip::PipBackend, scoop::ScoopBackend,
    uv::UvBackend, winget::WingetBackend,
};
use super::checker::{CheckTarget, UncheckedTool, VERSION_MANAGERS, version_manager_reason};
use super::config::MaintenanceConfig;

/// Everything the resolver needs from the outside world. Kept
/// primitive so this module compiles against `jarvy` the library
/// (which doesn't own `Config`). The binary-side caller in
/// `src/main.rs` builds this from the parsed `Config`; test code
/// constructs it directly without going through TOML.
#[derive(Debug, Default, Clone)]
pub struct ResolveInput {
    /// `(tool_name, pinned_version)` from `[provisioner]`.
    pub provisioner_tools: Vec<String>,
    /// Bare package names from `[cargo]`.
    pub cargo_packages: Vec<String>,
    /// Bare package names from `[npm]`.
    pub npm_packages: Vec<String>,
    /// Bare package names from `[pip]`.
    pub pip_packages: Vec<String>,
    /// Bare gem names from `[gem]`.
    pub gem_packages: Vec<String>,
    /// Fully-qualified Go module paths from `[go]` (e.g.
    /// `github.com/x/y`).
    pub go_packages: Vec<String>,
    /// `[nuget]` global tool names (e.g. `dotnet-ef`).
    pub nuget_packages: Vec<String>,
    /// Optional `{binary_name: installed_version}` map for the
    /// `[cargo]` section — populated once via `cargo install
    /// --list` by the caller so we don't spawn `<name> --version`
    /// per package (which mis-detects renamed binaries like
    /// `cargo-nextest` → `nextest`).
    pub cargo_installed: HashMap<String, String>,
}

/// How aggressively the resolver should probe the local
/// filesystem while building `CheckTarget`s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveMode {
    /// Full resolution — spawn `<cmd> --version` (up to 3 fallbacks)
    /// for every configured package so `CheckTarget.installed`
    /// carries the currently-installed version. Used by `jarvy
    /// check-updates` where the direction (up_to_date / upgrade /
    /// downgrade) matters.
    Full,
    /// Cache-only — skip the installed-version probe and set
    /// `installed: None` on every target. Used by `jarvy setup`
    /// which only needs the *count* of stale tools from cache
    /// entries. Closes the perf F1 hot-path finding: setup no
    /// longer pays 3 × N subprocess spawns on a config with N
    /// tools.
    Cheap,
}

/// Resolve every checkable target in `config`, plus the buckets of
/// tools we're deliberately skipping.
///
/// The caller decides whether to run the checker (they hold the
/// cache + telemetry gate). This function is pure I/O over the
/// registry + a couple of `--version` shell-outs for installed
/// versions — no network, no cache mutation, no telemetry
/// emission.
pub fn resolve_targets(
    input: &ResolveInput,
    maintenance: &MaintenanceConfig,
) -> (Vec<CheckTarget>, Vec<UncheckedTool>) {
    resolve_targets_with_mode(input, maintenance, ResolveMode::Full)
}

/// Cache-only variant of [`resolve_targets`]. Every returned
/// `CheckTarget` has `installed: None`; the setup summary path
/// only needs the aggregate stale count from cache entries, and
/// spawning `<cmd> --version` × N on every `jarvy setup` was the
/// P0 latency regression the PRD-057 review flagged.
pub fn resolve_targets_cheap(
    input: &ResolveInput,
    maintenance: &MaintenanceConfig,
) -> (Vec<CheckTarget>, Vec<UncheckedTool>) {
    resolve_targets_with_mode(input, maintenance, ResolveMode::Cheap)
}

fn resolve_targets_with_mode(
    input: &ResolveInput,
    maintenance: &MaintenanceConfig,
    mode: ResolveMode,
) -> (Vec<CheckTarget>, Vec<UncheckedTool>) {
    let mut targets: Vec<CheckTarget> = Vec::new();
    let mut unchecked: Vec<UncheckedTool> = Vec::new();
    let ignored: HashSet<&str> = maintenance.ignore.iter().map(|s| s.as_str()).collect();
    let mut seen: HashSet<String> = HashSet::new();
    // `backend:pkg_id` dedup across ALL push sites — a receipt-routed
    // provisioner tool must not double-probe when the same package
    // also appears in its ecosystem section (provisioner loop runs
    // first, so the receipt-routed target wins).
    let mut seen_pkg: HashSet<String> = HashSet::new();
    // PRD-060 Phase 2: still-valid fallback install receipts. One
    // disk read + a stat per receipt — cheap enough for both modes.
    let receipts = crate::tools::receipts::load_valid();

    // 1. Provisioner tools.
    for name in &input.provisioner_tools {
        if !seen.insert(name.clone()) {
            continue;
        }
        if ignored.contains(name.as_str()) {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "ignored_by_config".to_string(),
                manager: None,
            });
            continue;
        }
        if let Some(reason) = version_manager_reason(name) {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: reason.to_string(),
                manager: version_manager_match(name).map(String::from),
            });
            continue;
        }

        let Some(spec) = find_spec(name) else {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "unregistered_tool".to_string(),
                manager: None,
            });
            continue;
        };

        // PRD-060 Phase 2: a still-valid fallback receipt wins over
        // BOTH the custom_install bucket and the platform backend —
        // the binary on PATH is the ecosystem-installed one, so
        // asking brew/winget about it produces garbage. Receipt
        // strings are hostile input (user-writable file): the package
        // passes the same gauntlet as ecosystem TOML keys before it
        // can reach a backend argv. Unknown route strings fall
        // through to normal platform routing.
        if let Some(receipt) = crate::tools::receipts::lookup(&receipts, name)
            && let Some(backend) = receipt_backend(&receipt.route)
        {
            if receipt_package_safe(&receipt.route, &receipt.package) {
                let installed = if mode == ResolveMode::Full {
                    detect_installed_version(spec.command)
                } else {
                    None
                };
                push_target(
                    &mut targets,
                    &mut seen_pkg,
                    CheckTarget {
                        tool: name.clone(),
                        backend,
                        pkg_id: receipt.package.clone(),
                        installed,
                    },
                );
            } else {
                if crate::observability::telemetry_gate::is_enabled() {
                    tracing::warn!(
                        event = "maintenance.refused_unsafe_name",
                        purpose = "[receipt]",
                        reason = "receipt_package_invalid",
                    );
                }
                unchecked.push(UncheckedTool {
                    tool: name.clone(),
                    reason: "refused_unsafe_name".to_string(),
                    manager: None,
                });
            }
            continue;
        }

        if spec.custom_install.is_some() {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "custom_install".to_string(),
                manager: None,
            });
            continue;
        }
        match provisioner_backend(spec) {
            Some((backend, pkg_id)) => {
                let installed = if mode == ResolveMode::Full {
                    detect_installed_version(spec.command)
                } else {
                    None
                };
                push_target(
                    &mut targets,
                    &mut seen_pkg,
                    CheckTarget {
                        tool: name.clone(),
                        backend,
                        pkg_id,
                        installed,
                    },
                );
            }
            None => unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "no_backend_for_platform".to_string(),
                manager: None,
            }),
        }
    }

    // 2. [cargo] packages. Prefer the pre-populated
    // `cargo install --list` map (handles renamed binaries), fall
    // back to `<name> --version` for entries not in the list.
    for name in &input.cargo_packages {
        if !seen.insert(name.clone()) {
            continue;
        }
        if ignored.contains(name.as_str()) {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "ignored_by_config".to_string(),
                manager: None,
            });
            continue;
        }
        if !push_if_safe(name, "[maintenance-cargo]", &mut unchecked) {
            continue;
        }
        let installed = if mode == ResolveMode::Full {
            input
                .cargo_installed
                .get(name)
                .cloned()
                .or_else(|| detect_installed_version(name))
        } else {
            None
        };
        push_target(
            &mut targets,
            &mut seen_pkg,
            CheckTarget {
                tool: name.clone(),
                backend: Box::new(CargoBackend),
                pkg_id: name.clone(),
                installed,
            },
        );
    }

    // 3. [npm] packages. Detect installed globals in one shot so
    // we don't fan out N slow `npm view` invocations for the
    // `installed` side — `npm ls -g --json --depth=0` returns the
    // full map in a single subprocess. Cheap mode skips this too
    // (perf F1: no subprocess on the setup summary path).
    let npm_globals = if mode != ResolveMode::Full || input.npm_packages.is_empty() {
        HashMap::new()
    } else {
        detect_npm_globals()
    };
    for name in &input.npm_packages {
        if !seen.insert(name.clone()) {
            continue;
        }
        if ignored.contains(name.as_str()) {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "ignored_by_config".to_string(),
                manager: None,
            });
            continue;
        }
        if !push_if_safe(name, "[maintenance-npm]", &mut unchecked) {
            continue;
        }
        push_target(
            &mut targets,
            &mut seen_pkg,
            CheckTarget {
                tool: name.clone(),
                backend: Box::new(NpmBackend),
                pkg_id: name.clone(),
                installed: npm_globals.get(name).cloned(),
            },
        );
    }

    // 4. [pip] packages.
    for name in &input.pip_packages {
        if !seen.insert(name.clone()) {
            continue;
        }
        if ignored.contains(name.as_str()) {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "ignored_by_config".to_string(),
                manager: None,
            });
            continue;
        }
        if !push_if_safe(name, "[maintenance-pip]", &mut unchecked) {
            continue;
        }
        let installed = if mode == ResolveMode::Full {
            detect_installed_version(name)
        } else {
            None
        };
        push_target(
            &mut targets,
            &mut seen_pkg,
            CheckTarget {
                tool: name.clone(),
                backend: Box::new(PipBackend),
                pkg_id: name.clone(),
                installed,
            },
        );
    }

    // 5. [gem] packages.
    for name in &input.gem_packages {
        if !seen.insert(name.clone()) {
            continue;
        }
        if ignored.contains(name.as_str()) {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "ignored_by_config".to_string(),
                manager: None,
            });
            continue;
        }
        if !push_if_safe(name, "[maintenance-gem]", &mut unchecked) {
            continue;
        }
        let installed = if mode == ResolveMode::Full {
            detect_installed_version(name)
        } else {
            None
        };
        push_target(
            &mut targets,
            &mut seen_pkg,
            CheckTarget {
                tool: name.clone(),
                backend: Box::new(GemBackend),
                pkg_id: name.clone(),
                installed,
            },
        );
    }

    // 6. [go] packages. Bare command names are inferred from the
    // last path segment for the installed-version probe — a `go
    // install` places the binary there. Backend queries the full
    // module path.
    for path in &input.go_packages {
        if !seen.insert(path.clone()) {
            continue;
        }
        if ignored.contains(path.as_str()) {
            unchecked.push(UncheckedTool {
                tool: path.clone(),
                reason: "ignored_by_config".to_string(),
                manager: None,
            });
            continue;
        }
        // Go module keys additionally cannot contain `..` segments —
        // `validate_package_name` accepts `.` in the safe set (needed
        // for legitimate `github.com/foo/bar.v1` style paths), but a
        // `..` segment lets `go_binary_name`'s last-segment probe
        // resolve outside the intended package tree.
        if !push_if_safe(path, "[maintenance-go]", &mut unchecked) {
            continue;
        }
        if !is_safe_go_module_path(path) {
            unchecked.push(UncheckedTool {
                tool: path.clone(),
                reason: "refused_unsafe_name".to_string(),
                manager: None,
            });
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "maintenance.refused_unsafe_name",
                    purpose = "[maintenance-go]",
                    reason = "traversal_segment",
                );
            }
            continue;
        }
        let installed = if mode == ResolveMode::Full {
            let bin_name = go_binary_name(path);
            detect_installed_version(bin_name)
        } else {
            None
        };
        push_target(
            &mut targets,
            &mut seen_pkg,
            CheckTarget {
                tool: path.clone(),
                backend: Box::new(GoBackend),
                pkg_id: path.clone(),
                installed,
            },
        );
    }

    // 7. [nuget] global tools.
    for name in &input.nuget_packages {
        if !seen.insert(name.clone()) {
            continue;
        }
        if ignored.contains(name.as_str()) {
            unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "ignored_by_config".to_string(),
                manager: None,
            });
            continue;
        }
        if !push_if_safe(name, "[maintenance-nuget]", &mut unchecked) {
            continue;
        }
        // dotnet global tools install into `~/.dotnet/tools/`
        // and expose `<tool> --version`. Trust the command probe
        // as it's the same convention every dotnet tool follows.
        let installed = if mode == ResolveMode::Full {
            detect_installed_version(name)
        } else {
            None
        };
        push_target(
            &mut targets,
            &mut seen_pkg,
            CheckTarget {
                tool: name.clone(),
                backend: Box::new(NugetBackend),
                pkg_id: name.clone(),
                installed,
            },
        );
    }

    (targets, unchecked)
}

/// Push a target unless the same `backend:pkg_id` pair is already
/// queued — first declaration wins. Silent skip, not an `unchecked`
/// row: the package IS being checked, just under its earlier target.
fn push_target(
    targets: &mut Vec<CheckTarget>,
    seen_pkg: &mut HashSet<String>,
    target: CheckTarget,
) {
    let key = format!("{}:{}", target.backend.name(), target.pkg_id);
    if seen_pkg.insert(key) {
        targets.push(target);
    }
}

/// Map a receipt's route string onto its freshness backend. `None`
/// for anything unrecognized (hand-edited or future-schema receipt) —
/// caller falls through to normal platform routing.
fn receipt_backend(route: &str) -> Option<Box<dyn FreshnessBackend + Send + Sync>> {
    match route {
        "go" => Some(Box::new(GoBackend)),
        "npm" => Some(Box::new(NpmBackend)),
        "cargo" => Some(Box::new(CargoBackend)),
        "uv" => Some(Box::new(UvBackend)),
        _ => None,
    }
}

/// Receipt package strings run the same gauntlet as ecosystem TOML
/// keys (the receipts file is user-writable): charset validation,
/// path-shape refusal, and for go routes the `..` segment check.
fn receipt_package_safe(route: &str, package: &str) -> bool {
    !(package.starts_with('/') || package.starts_with('\\'))
        && validate_package_name(package, "[receipt]").is_ok()
        && (route != "go" || is_safe_go_module_path(package))
}

/// Gate an ecosystem-key against [`validate_package_name`] and, on
/// refusal, bucket it as `unchecked` with the bounded telemetry
/// label `refused_unsafe_name`. Returns `true` when the caller
/// should proceed with target construction.
///
/// This is the single load-bearing security gate for the checker's
/// language-scoped sections ([cargo] / [npm] / [pip] / [gem] / [go]
/// / [nuget]) — TOML keys flow from an attacker-controllable file
/// directly into `Command::new` (via `detect_installed_version`) and
/// as argv elements to package managers (via each backend). Without
/// this gate a hostile local `jarvy.toml` executes arbitrary
/// binaries and injects registry-redirect flags. The provisioner
/// path routes through the registered tool spec instead, so it does
/// not need this check.
///
/// On top of [`validate_package_name`] this helper also refuses
/// names starting with `/` or `\` — legal per the safe charset
/// (which permits `/` for npm scoped names like `@scope/pkg`) but
/// path-shaped inputs are never a legitimate package identifier
/// and blocking them at the resolver keeps `detect_installed_version`
/// and every backend's argv one layer away from a filesystem lookup.
fn push_if_safe(name: &str, purpose: &'static str, unchecked: &mut Vec<UncheckedTool>) -> bool {
    let path_shaped = name.starts_with('/') || name.starts_with('\\');
    if !path_shaped && validate_package_name(name, purpose).is_ok() {
        return true;
    }
    if path_shaped && crate::observability::telemetry_gate::is_enabled() {
        tracing::warn!(
            event = "maintenance.refused_unsafe_name",
            purpose = %purpose,
            reason = "path_shaped_key",
        );
    }
    unchecked.push(UncheckedTool {
        tool: name.to_string(),
        reason: "refused_unsafe_name".to_string(),
        manager: None,
    });
    false
}

/// Refuse any Go module path segment that resolves outside the
/// declared tree. `validate_package_name` accepts `.` in the safe
/// charset (needed for `github.com/x/y.v1`-style names); it does
/// not reject a `..` segment. This helper closes that gap.
fn is_safe_go_module_path(path: &str) -> bool {
    !path.split('/').any(|seg| seg == ".." || seg == ".")
}

/// Extract the plausible binary name for a Go module path.
/// `github.com/foo/bar` → `bar`; `github.com/foo/bar/v2` → `bar`
/// (Go's SIV convention strips the version suffix from the
/// binary name).
fn go_binary_name(path: &str) -> &str {
    let last = path.rsplit('/').next().unwrap_or(path);
    // Strip semantic-import-versioning suffixes like `/v2`, `/v3`.
    if last.starts_with('v') && last.len() >= 2 && last[1..].chars().all(|c| c.is_ascii_digit()) {
        path.rsplit('/').nth(1).unwrap_or(last)
    } else {
        last
    }
}

/// Shell out to `cargo install --list` and return a
/// `{crate_name: version}` map. Cargo's output shape is:
///
/// ```text
/// cargo-nextest v0.9.72:
///     nextest
/// ripgrep v14.1.0:
///     rg
/// ```
///
/// One entry per installed crate + an indented list of installed
/// binary names. The map is keyed by CRATE name (the [cargo]
/// section's key) not by binary name — the caller passes crate
/// names, and this matches the section's declaration style. Errors
/// return an empty map; the checker falls back to `<name>
/// --version` for missing entries.
pub fn detect_cargo_installed() -> HashMap<String, String> {
    let Ok(out) = Command::new("cargo").args(["install", "--list"]).output() else {
        return HashMap::new();
    };
    if !out.status.success() {
        return HashMap::new();
    }
    let Ok(text) = std::str::from_utf8(&out.stdout) else {
        return HashMap::new();
    };
    parse_cargo_install_list(text)
}

fn parse_cargo_install_list(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        // Header lines start at column 0 and end with `:`;
        // binary lines are indented.
        if line.starts_with(' ') || line.starts_with('\t') || line.trim().is_empty() {
            continue;
        }
        let trimmed = line.trim_end_matches(':').trim();
        // Split on the LAST whitespace so crate names carrying
        // internal spaces (rare but legal) still parse.
        let Some((name, ver)) = trimmed.rsplit_once(' ') else {
            continue;
        };
        let ver = ver.strip_prefix('v').unwrap_or(ver);
        if ver.is_empty() || name.is_empty() {
            continue;
        }
        map.insert(name.to_string(), ver.to_string());
    }
    map
}

/// Shell out to `npm ls -g --json --depth=0` and return
/// `{package_name: version}` for every top-level global. Errors
/// return an empty map — the caller treats missing entries as
/// `installed = None`.
fn detect_npm_globals() -> HashMap<String, String> {
    let Ok(out) = Command::new("npm")
        .args(["ls", "-g", "--json", "--depth=0"])
        .output()
    else {
        return HashMap::new();
    };
    if out.stdout.is_empty() {
        return HashMap::new();
    }
    // npm may exit non-zero on peer-dep warnings but the JSON body
    // is still complete — parse regardless of exit code.
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return HashMap::new();
    };
    let Some(deps) = value.get("dependencies").and_then(|d| d.as_object()) else {
        return HashMap::new();
    };
    deps.iter()
        .filter_map(|(name, entry)| {
            let ver = entry.get("version").and_then(|v| v.as_str())?;
            Some((name.clone(), ver.to_string()))
        })
        .collect()
}

/// Route a `ToolSpec` to a freshness backend for the current OS.
/// Returns `None` when no supported backend has a package ID for
/// this platform — that's a signal to bucket the tool as unchecked
/// rather than emit a spurious "not found" error.
///
/// Per-OS preference order matches the installer's runtime picker
/// (brew on macOS; native pkg mgr → linuxbrew on Linux; winget →
/// choco → scoop on Windows) so a freshness result actually
/// corresponds to what `jarvy setup` would install.
fn provisioner_backend(
    spec: &ToolSpec,
) -> Option<(Box<dyn FreshnessBackend + Send + Sync>, String)> {
    #[cfg(target_os = "macos")]
    {
        if let Some(mac) = spec.macos {
            if let Some(formula) = mac.brew {
                return Some((Box::new(BrewBackend), formula.to_string()));
            }
            if let Some(cask) = mac.cask {
                return Some((Box::new(BrewBackend), cask.to_string()));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(linux) = spec.linux {
            // Prefer the native package manager whose CLI is on
            // PATH — that's the one setup would actually invoke.
            // Fall back to linuxbrew when nothing native is
            // available.
            if let Some(pkg) = linux.apt
                && crate::tools::common::command_on_path("apt-cache")
            {
                return Some((Box::new(AptBackend), pkg.to_string()));
            }
            if let Some(pkg) = linux.dnf
                && crate::tools::common::command_on_path("dnf")
            {
                return Some((Box::new(DnfBackend), pkg.to_string()));
            }
            if let Some(pkg) = linux.yum
                && crate::tools::common::command_on_path("dnf")
            {
                // dnf reads yum repos too — reuse the dnf backend
                // for yum-flavored distros.
                return Some((Box::new(DnfBackend), pkg.to_string()));
            }
            if let Some(pkg) = linux.pacman
                && crate::tools::common::command_on_path("pacman")
            {
                return Some((Box::new(PacmanBackend), pkg.to_string()));
            }
            if let Some(pkg) = linux.apk
                && crate::tools::common::command_on_path("apk")
            {
                return Some((Box::new(ApkBackend), pkg.to_string()));
            }
            if let Some(formula) = linux.brew {
                return Some((Box::new(BrewBackend), formula.to_string()));
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(win) = spec.windows {
            if let Some(id) = win.winget
                && crate::tools::common::command_on_path("winget")
            {
                return Some((Box::new(WingetBackend), id.to_string()));
            }
            if let Some(pkg) = win.choco
                && crate::tools::common::command_on_path("choco")
            {
                return Some((Box::new(ChocoBackend), pkg.to_string()));
            }
            // Prefer the explicit `scoop:` field on WindowsInstall
            // when set. Only when the tool never declared a scoop
            // bucket name AND we still want to attempt a lookup
            // do we fall back to the winget id — this is best-
            // effort and returns NotFound when the bucket name
            // diverges.
            if let Some(pkg) = win.scoop
                && crate::tools::common::command_on_path("scoop")
            {
                return Some((Box::new(ScoopBackend), pkg.to_string()));
            }
            if let Some(id) = win.winget
                && win.scoop.is_none()
                && crate::tools::common::command_on_path("scoop")
            {
                return Some((Box::new(ScoopBackend), id.to_string()));
            }
        }
    }
    let _ = spec;
    None
}

fn find_spec(name: &str) -> Option<&'static ToolSpec> {
    // Same aliasing as before, but O(1) via the shared spec map instead
    // of a per-call linear scan over ~180 specs.
    crate::tools::spec::get_tool_spec(name)
}

fn version_manager_match(name: &str) -> Option<&'static str> {
    VERSION_MANAGERS
        .iter()
        .copied()
        .find(|vm| name == *vm || name.contains(vm))
}

/// Best-effort `<cmd> --version` probe. Uses the shared version
/// extractor so downstream comparisons see the same normalized
/// shape drift + lock output produce. Returns `None` when the
/// binary isn't on PATH or the output can't be parsed — the
/// checker treats `None` as "installed version unknown" and the
/// backend result becomes advisory rather than a comparison.
///
/// Defense-in-depth on top of the resolver's `push_if_safe` gate:
/// refuse any `cmd` that isn't a bare binary name. `Command::new`
/// resolves absolute paths and relative paths against cwd, so
/// `./payload` or `../../etc/reboot` would exec arbitrary code if
/// a bug in the caller ever bypassed the resolver's validator.
/// Every legitimate call site passes either a `ToolSpec.command`
/// static string (safe) or a `push_if_safe`-validated ecosystem
/// key (safe); this final guard closes the case where a future
/// refactor forgets one.
fn detect_installed_version(cmd: &str) -> Option<String> {
    if cmd.is_empty()
        || cmd.starts_with('-')
        || cmd.contains('/')
        || cmd.contains('\\')
        || cmd
            .chars()
            .any(|c| c.is_control() || c == '\x1b' || c == '\x7f')
    {
        return None;
    }
    // EP-12: cap each probe at 5s via `probe_with_timeout`. A `Command::new(cmd).output()`
    // chain can hang forever if the tool spawns a background daemon or blocks on stdin
    // (cobra CLIs, node CLIs that lazily connect to a registry) — a maintenance sweep of
    // 40 tools then hangs the setup phase. `cmd_version_output` already uses this helper
    // for `jarvy doctor`; the maintenance sweep needed the same guarantee.
    use crate::tools::common::{ProbeResult, probe_with_timeout};
    const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
    for flag in ["--version", "-V", "version"] {
        match probe_with_timeout(cmd, &[flag], PROBE_TIMEOUT) {
            ProbeResult::Completed(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(v) = crate::tools::version::extract_version(&text) {
                    return Some(v.to_string());
                }
            }
            ProbeResult::Missing | ProbeResult::PermissionDenied => return None,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_no_targets() {
        let input = ResolveInput::default();
        let maintenance = MaintenanceConfig::default();
        let (targets, unchecked) = resolve_targets(&input, &maintenance);
        assert!(targets.is_empty());
        assert!(unchecked.is_empty());
    }

    #[test]
    fn ignored_tools_land_in_unchecked() {
        let input = ResolveInput {
            provisioner_tools: vec!["jq".to_string()],
            ..Default::default()
        };
        let maintenance = MaintenanceConfig {
            ignore: vec!["jq".to_string()],
            ..Default::default()
        };
        let (targets, unchecked) = resolve_targets(&input, &maintenance);
        assert!(targets.is_empty());
        assert_eq!(unchecked.len(), 1);
        assert_eq!(unchecked[0].reason, "ignored_by_config");
    }

    #[test]
    fn cargo_install_list_parses_crate_and_version() {
        let text = "cargo-nextest v0.9.72:\n    nextest\nripgrep v14.1.0:\n    rg\n";
        let map = parse_cargo_install_list(text);
        assert_eq!(map.get("cargo-nextest").map(String::as_str), Some("0.9.72"));
        assert_eq!(map.get("ripgrep").map(String::as_str), Some("14.1.0"));
    }

    #[test]
    fn cargo_install_list_ignores_empty_and_indented() {
        let text = "\n    nextest\ncargo-watch v8.5.2:\n    cargo-watch\n";
        let map = parse_cargo_install_list(text);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("cargo-watch").map(String::as_str), Some("8.5.2"));
    }

    #[test]
    fn go_binary_name_strips_siv_suffix() {
        assert_eq!(go_binary_name("github.com/foo/bar"), "bar");
        assert_eq!(go_binary_name("github.com/foo/bar/v2"), "bar");
        assert_eq!(go_binary_name("github.com/foo/bar/v10"), "bar");
    }

    #[test]
    fn version_manager_bucketed() {
        let input = ResolveInput {
            provisioner_tools: vec!["rustup".to_string()],
            ..Default::default()
        };
        let maintenance = MaintenanceConfig::default();
        let (_targets, unchecked) = resolve_targets(&input, &maintenance);
        assert!(unchecked.iter().any(|u| u.tool == "rustup"));
    }

    // Security regression pins — every ecosystem loop must refuse
    // adversarial TOML keys before they reach `Command::new` (via
    // `detect_installed_version`) or the backend argv (via
    // `pkg_id`).
    #[test]
    fn cargo_leading_dash_key_is_refused_as_unsafe_name() {
        let input = ResolveInput {
            cargo_packages: vec!["--registry=http://evil.example/".to_string()],
            ..Default::default()
        };
        let (targets, unchecked) = resolve_targets(&input, &MaintenanceConfig::default());
        assert!(targets.is_empty(), "no target should be built");
        assert_eq!(unchecked.len(), 1);
        assert_eq!(unchecked[0].reason, "refused_unsafe_name");
    }

    #[test]
    fn npm_absolute_path_key_is_refused() {
        let input = ResolveInput {
            npm_packages: vec!["/usr/bin/reboot".to_string()],
            ..Default::default()
        };
        let (targets, unchecked) = resolve_targets(&input, &MaintenanceConfig::default());
        assert!(targets.is_empty());
        assert_eq!(unchecked[0].reason, "refused_unsafe_name");
    }

    #[test]
    fn pip_url_scheme_key_is_refused() {
        let input = ResolveInput {
            pip_packages: vec!["https://evil.example/pypi".to_string()],
            ..Default::default()
        };
        let (_, unchecked) = resolve_targets(&input, &MaintenanceConfig::default());
        assert_eq!(unchecked[0].reason, "refused_unsafe_name");
    }

    #[test]
    fn gem_control_byte_key_is_refused() {
        let input = ResolveInput {
            gem_packages: vec!["evil\x1b[2Jgem".to_string()],
            ..Default::default()
        };
        let (targets, unchecked) = resolve_targets(&input, &MaintenanceConfig::default());
        assert!(targets.is_empty());
        assert_eq!(unchecked[0].reason, "refused_unsafe_name");
    }

    #[test]
    fn nuget_flag_injection_key_is_refused() {
        let input = ResolveInput {
            nuget_packages: vec!["--tool-path=/tmp/pwn".to_string()],
            ..Default::default()
        };
        let (_, unchecked) = resolve_targets(&input, &MaintenanceConfig::default());
        assert_eq!(unchecked[0].reason, "refused_unsafe_name");
    }

    #[test]
    fn go_traversal_segment_is_refused() {
        let input = ResolveInput {
            go_packages: vec!["github.com/x/../../etc/passwd".to_string()],
            ..Default::default()
        };
        let (targets, unchecked) = resolve_targets(&input, &MaintenanceConfig::default());
        assert!(targets.is_empty());
        assert_eq!(unchecked[0].reason, "refused_unsafe_name");
    }

    #[test]
    fn go_current_directory_segment_is_refused() {
        // A `.` segment resolves to cwd; go_binary_name would then
        // pick up "./" as the binary name. Refuse for the same
        // reason as `..`.
        let input = ResolveInput {
            go_packages: vec!["github.com/x/./evil".to_string()],
            ..Default::default()
        };
        let (targets, unchecked) = resolve_targets(&input, &MaintenanceConfig::default());
        assert!(targets.is_empty());
        assert_eq!(unchecked[0].reason, "refused_unsafe_name");
    }

    #[test]
    fn detect_installed_version_refuses_path_shaped_cmd() {
        // Defense-in-depth: even if a future caller forgets the
        // resolver gate, the probe itself refuses to exec anything
        // that isn't a bare binary name.
        assert_eq!(detect_installed_version("../../bin/reboot"), None);
        assert_eq!(detect_installed_version("/usr/bin/reboot"), None);
        assert_eq!(detect_installed_version("--version"), None);
        assert_eq!(detect_installed_version(""), None);
        assert_eq!(detect_installed_version("evil\x1bcmd"), None);
    }

    #[test]
    fn safe_go_module_path_accepts_legitimate_names() {
        assert!(is_safe_go_module_path("github.com/foo/bar"));
        assert!(is_safe_go_module_path("github.com/foo/bar/v2"));
        assert!(is_safe_go_module_path("gopkg.in/yaml.v3"));
        assert!(!is_safe_go_module_path("github.com/x/.."));
        assert!(!is_safe_go_module_path("../../../evil"));
    }

    #[test]
    fn receipt_backend_maps_known_routes_only() {
        for (route, name) in [
            ("go", "go"),
            ("npm", "npm"),
            ("cargo", "cargo"),
            ("uv", "uv"),
        ] {
            let backend = receipt_backend(route).expect(route);
            assert_eq!(backend.name(), name);
        }
        assert!(receipt_backend("pip").is_none());
        assert!(receipt_backend("curl-sh").is_none());
        assert!(receipt_backend("").is_none());
    }

    #[test]
    fn receipt_package_gauntlet_refuses_hostile_strings() {
        assert!(receipt_package_safe(
            "go",
            "github.com/betterleaks/betterleaks"
        ));
        assert!(receipt_package_safe("uv", "cfn-lint"));
        assert!(receipt_package_safe("npm", "@scope/pkg"));
        // Path-shaped, traversal, and injection shapes refused.
        assert!(!receipt_package_safe("uv", "/usr/bin/evil"));
        assert!(!receipt_package_safe("npm", "\\\\share\\evil"));
        assert!(!receipt_package_safe("go", "github.com/x/../../etc"));
        assert!(!receipt_package_safe("cargo", "--registry=evil"));
        assert!(!receipt_package_safe("uv", "pkg; rm -rf ~"));
    }

    #[test]
    fn push_target_dedups_backend_pkg_pairs() {
        let mut targets = Vec::new();
        let mut seen_pkg = HashSet::new();
        for tool in ["betterleaks", "dup-entry"] {
            push_target(
                &mut targets,
                &mut seen_pkg,
                CheckTarget {
                    tool: tool.to_string(),
                    backend: Box::new(GoBackend),
                    pkg_id: "github.com/betterleaks/betterleaks".to_string(),
                    installed: None,
                },
            );
        }
        // Same pkg on a DIFFERENT backend is not a dup.
        push_target(
            &mut targets,
            &mut seen_pkg,
            CheckTarget {
                tool: "other".to_string(),
                backend: Box::new(NpmBackend),
                pkg_id: "github.com/betterleaks/betterleaks".to_string(),
                installed: None,
            },
        );
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].tool, "betterleaks");
    }
}
