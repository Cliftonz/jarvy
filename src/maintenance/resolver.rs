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

use crate::tools::spec::{ToolSpec, iter_tools};

// Backend imports — several are per-OS via `#[cfg(...)]` branches
// in `provisioner_backend`. Rust's dead-code check runs after cfg
// pruning, so the whole list is gated with `allow(unused_imports)`
// rather than a fan-out of per-cfg `use` blocks that would drift.
#[allow(unused_imports)]
use super::backends::{
    FreshnessBackend, apk::ApkBackend, apt::AptBackend, brew::BrewBackend, cargo::CargoBackend,
    choco::ChocoBackend, dnf::DnfBackend, gem::GemBackend, go::GoBackend, npm::NpmBackend,
    nuget::NugetBackend, pacman::PacmanBackend, pip::PipBackend, scoop::ScoopBackend,
    winget::WingetBackend,
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
    let mut targets: Vec<CheckTarget> = Vec::new();
    let mut unchecked: Vec<UncheckedTool> = Vec::new();
    let ignored: HashSet<&str> = maintenance.ignore.iter().map(|s| s.as_str()).collect();
    let mut seen: HashSet<String> = HashSet::new();

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

        let spec = find_spec(name);
        match spec {
            None => unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "unregistered_tool".to_string(),
                manager: None,
            }),
            Some(spec) if spec.custom_install.is_some() => unchecked.push(UncheckedTool {
                tool: name.clone(),
                reason: "custom_install".to_string(),
                manager: None,
            }),
            Some(spec) => match provisioner_backend(spec) {
                Some((backend, pkg_id)) => {
                    let installed = detect_installed_version(spec.command);
                    targets.push(CheckTarget {
                        tool: name.clone(),
                        backend,
                        pkg_id,
                        installed,
                    });
                }
                None => unchecked.push(UncheckedTool {
                    tool: name.clone(),
                    reason: "no_backend_for_platform".to_string(),
                    manager: None,
                }),
            },
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
        let installed = input
            .cargo_installed
            .get(name)
            .cloned()
            .or_else(|| detect_installed_version(name));
        targets.push(CheckTarget {
            tool: name.clone(),
            backend: Box::new(CargoBackend),
            pkg_id: name.clone(),
            installed,
        });
    }

    // 3. [npm] packages. Detect installed globals in one shot so
    // we don't fan out N slow `npm view` invocations for the
    // `installed` side — `npm ls -g --json --depth=0` returns the
    // full map in a single subprocess.
    let npm_globals = if input.npm_packages.is_empty() {
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
        targets.push(CheckTarget {
            tool: name.clone(),
            backend: Box::new(NpmBackend),
            pkg_id: name.clone(),
            installed: npm_globals.get(name).cloned(),
        });
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
        let installed = detect_installed_version(name);
        targets.push(CheckTarget {
            tool: name.clone(),
            backend: Box::new(PipBackend),
            pkg_id: name.clone(),
            installed,
        });
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
        let installed = detect_installed_version(name);
        targets.push(CheckTarget {
            tool: name.clone(),
            backend: Box::new(GemBackend),
            pkg_id: name.clone(),
            installed,
        });
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
        let bin_name = go_binary_name(path);
        let installed = detect_installed_version(bin_name);
        targets.push(CheckTarget {
            tool: path.clone(),
            backend: Box::new(GoBackend),
            pkg_id: path.clone(),
            installed,
        });
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
        // dotnet global tools install into `~/.dotnet/tools/`
        // and expose `<tool> --version`. Trust the command probe
        // as it's the same convention every dotnet tool follows.
        let installed = detect_installed_version(name);
        targets.push(CheckTarget {
            tool: name.clone(),
            backend: Box::new(NugetBackend),
            pkg_id: name.clone(),
            installed,
        });
    }

    (targets, unchecked)
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
    let key = name.to_ascii_lowercase();
    let alias_dash = key.replace('_', "-");
    let alias_underscore = key.replace('-', "_");
    iter_tools().map(|entry| entry.spec).find(|s| {
        let sname = s.name.to_ascii_lowercase();
        sname == key || sname == alias_dash || sname == alias_underscore
    })
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
fn detect_installed_version(cmd: &str) -> Option<String> {
    let out = Command::new(cmd)
        .arg("--version")
        .output()
        .or_else(|_| Command::new(cmd).arg("-V").output())
        .or_else(|_| Command::new(cmd).arg("version").output())
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    crate::tools::version::extract_version(&text).map(|v| v.to_string())
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
}
