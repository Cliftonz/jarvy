//! java - Java Development Kit
//!
//! Distribution-aware install path: honors `Tool.distribution`
//! (`openjdk` / `temurin` / `zulu` / `corretto` / `liberica` /
//! `microsoft-openjdk`) and `Tool.fallback` seeded by `setup_cmd` via
//! `tools::common::set_tool_override`. On unsupported (distribution,
//! OS) pairs the router warns and falls back to openjdk unless
//! `fallback = false`, in which case it hard-errors via
//! `InstallError::Prereq`.

use crate::define_tool;
use crate::tools::common::{InstallError, get_tool_override, has, run};

define_tool!(JAVA, {
    command: "java",
    macos: { brew: "openjdk" },
    linux: { apt: "default-jdk", dnf: "java-latest-openjdk", pacman: "jdk-openjdk", apk: "openjdk21" },
    windows: { winget: "Microsoft.OpenJDK.21", choco: "openjdk" },
    bsd: { pkg: "openjdk21" },
    custom_install: install_java,
    default_hook: {
        description: "Configure JAVA_HOME environment variable",
        script: r#"
# Set JAVA_HOME based on platform
if [ "$(uname)" = "Darwin" ]; then
    JAVA_HOME_PATH="$(/usr/libexec/java_home 2>/dev/null || true)"
elif [ -d "/usr/lib/jvm/default" ]; then
    JAVA_HOME_PATH="/usr/lib/jvm/default"
elif [ -d "/usr/lib/jvm/java-21-openjdk" ]; then
    JAVA_HOME_PATH="/usr/lib/jvm/java-21-openjdk"
fi

if [ -n "$JAVA_HOME_PATH" ]; then
    JAVA_EXPORT="export JAVA_HOME=\"$JAVA_HOME_PATH\""

    # Add to .bashrc if not present
    if [ -f "$HOME/.bashrc" ] && ! grep -q 'JAVA_HOME' "$HOME/.bashrc"; then
        echo "$JAVA_EXPORT" >> "$HOME/.bashrc"
        echo "Added JAVA_HOME to ~/.bashrc"
    fi

    # Add to .zshrc if not present
    if [ -f "$HOME/.zshrc" ] && ! grep -q 'JAVA_HOME' "$HOME/.zshrc"; then
        echo "$JAVA_EXPORT" >> "$HOME/.zshrc"
        echo "Added JAVA_HOME to ~/.zshrc"
    fi
fi
"#
    },
});

/// Bounded set of supported JDK distributions. Adding a new one must
/// land alongside a matching arm in every `resolve_*` router and a
/// new arm here — the closed enum is the only surface that guarantees
/// user input can't reach a package-name interpolation without
/// passing the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JdkDistribution {
    Openjdk,
    Temurin,
    Zulu,
    Corretto,
    Liberica,
    MicrosoftOpenjdk,
}

impl JdkDistribution {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "openjdk" => Some(Self::Openjdk),
            "temurin" | "adoptium" => Some(Self::Temurin),
            "zulu" | "azul" => Some(Self::Zulu),
            "corretto" | "amazon" => Some(Self::Corretto),
            "liberica" | "bellsoft" => Some(Self::Liberica),
            "microsoft-openjdk" | "msopenjdk" | "microsoft" => Some(Self::MicrosoftOpenjdk),
            _ => None,
        }
    }

    fn as_slug(self) -> &'static str {
        match self {
            Self::Openjdk => "openjdk",
            Self::Temurin => "temurin",
            Self::Zulu => "zulu",
            Self::Corretto => "corretto",
            Self::Liberica => "liberica",
            Self::MicrosoftOpenjdk => "microsoft-openjdk",
        }
    }
}

/// Parsed version selector. Anything the config passes that isn't
/// `""` / `"latest"` / a bare integer 8..=99 is silently coerced to
/// `Latest` — prevents `version = "17; rm -rf /"` from reaching a
/// `format!("openjdk@{}", version)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VersionSelector {
    Latest,
    Major(u32),
}

fn normalize_version(v: &str) -> VersionSelector {
    let t = v.trim().to_ascii_lowercase();
    if t.is_empty() || t == "latest" {
        return VersionSelector::Latest;
    }
    if let Ok(n) = t.parse::<u32>()
        && (8..=99).contains(&n)
    {
        return VersionSelector::Major(n);
    }
    VersionSelector::Latest
}

/// Per-OS install route. `Unsupported` means: no first-party route
/// modeled for this (distribution, OS) pair — caller warns and either
/// falls back to openjdk (default) or errors (`fallback = false`).
///
/// Variants are cfg-conditionally constructed by `resolve_macos` /
/// `resolve_linux` / `resolve_windows`; the `#[allow(dead_code)]`
/// keeps single-platform builds quiet without hiding real unreachable
/// code — every arm has a corresponding `execute_route` handler.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum InstallRoute {
    BrewFormula(String),
    BrewCask(String),
    AptPkg(String),
    DnfPkg(String),
    Winget(String),
    Choco(String),
    Unsupported { reason: String },
}

#[cfg(target_os = "macos")]
fn resolve_macos(distro: JdkDistribution, ver: VersionSelector) -> InstallRoute {
    use InstallRoute::*;
    use JdkDistribution::*;
    match (distro, ver) {
        (Openjdk, VersionSelector::Latest) => BrewFormula("openjdk".into()),
        (Openjdk, VersionSelector::Major(n)) => BrewFormula(format!("openjdk@{n}")),
        (Temurin, VersionSelector::Latest) => BrewCask("temurin".into()),
        (Temurin, VersionSelector::Major(n)) => BrewCask(format!("temurin@{n}")),
        (Zulu, VersionSelector::Latest) => BrewCask("zulu".into()),
        (Zulu, VersionSelector::Major(n)) => BrewCask(format!("zulu@{n}")),
        (Corretto, VersionSelector::Latest) => BrewCask("corretto".into()),
        (Corretto, VersionSelector::Major(n)) => BrewCask(format!("corretto@{n}")),
        (Liberica, VersionSelector::Latest) => BrewCask("liberica".into()),
        (Liberica, VersionSelector::Major(n)) => BrewCask(format!("liberica@{n}")),
        (MicrosoftOpenjdk, VersionSelector::Latest) => BrewCask("microsoft-openjdk".into()),
        (MicrosoftOpenjdk, VersionSelector::Major(n)) => BrewCask(format!("microsoft-openjdk@{n}")),
    }
}

#[cfg(target_os = "linux")]
fn resolve_linux(distro: JdkDistribution, ver: VersionSelector) -> InstallRoute {
    use InstallRoute::*;
    use JdkDistribution::*;
    // Detect PM at route-resolve time; only apt/dnf carry first-party
    // openjdk packages we're confident about. Others land in the
    // openjdk platform-default path via `install_platform`.
    let pm = crate::tools::common::detect_linux_pm();
    match distro {
        Openjdk => match (pm, ver) {
            (Some(crate::tools::common::PackageManager::Apt), VersionSelector::Major(n)) => {
                AptPkg(format!("openjdk-{n}-jdk"))
            }
            (Some(crate::tools::common::PackageManager::Apt), VersionSelector::Latest) => {
                AptPkg("default-jdk".into())
            }
            (Some(crate::tools::common::PackageManager::Dnf), VersionSelector::Major(n)) => {
                DnfPkg(format!("java-{n}-openjdk-devel"))
            }
            (Some(crate::tools::common::PackageManager::Dnf), VersionSelector::Latest) => {
                DnfPkg("java-latest-openjdk-devel".into())
            }
            (other_pm, _) => Unsupported {
                reason: format!(
                    "openjdk distribution not modeled for {other_pm:?} yet - falling back to jarvy default"
                ),
            },
        },
        // Third-party JDK vendors on Linux ship via their own apt/dnf
        // repos (or manual tarballs). Auto-configuring vendor repos is
        // out of scope for v1; caller either falls back to openjdk or
        // hard-errors per `fallback` flag.
        _ => Unsupported {
            reason: format!(
                "distribution '{}' on Linux requires the vendor's third-party apt/dnf repo; jarvy v1 does not auto-configure vendor repos",
                distro.as_slug()
            ),
        },
    }
}

#[cfg(target_os = "windows")]
fn resolve_windows(distro: JdkDistribution, ver: VersionSelector) -> InstallRoute {
    use InstallRoute::*;
    use JdkDistribution::*;
    match (distro, ver) {
        (Openjdk, VersionSelector::Latest) => Winget("Microsoft.OpenJDK.21".into()),
        (Openjdk, VersionSelector::Major(n)) => Winget(format!("Microsoft.OpenJDK.{n}")),
        (Temurin, VersionSelector::Latest) => Winget("EclipseAdoptium.Temurin.21.JDK".into()),
        (Temurin, VersionSelector::Major(n)) => Winget(format!("EclipseAdoptium.Temurin.{n}.JDK")),
        (Zulu, VersionSelector::Latest) => Winget("Azul.Zulu.21.JDK".into()),
        (Zulu, VersionSelector::Major(n)) => Winget(format!("Azul.Zulu.{n}.JDK")),
        (Corretto, VersionSelector::Latest) => Winget("Amazon.Corretto.21.JDK".into()),
        (Corretto, VersionSelector::Major(n)) => Winget(format!("Amazon.Corretto.{n}.JDK")),
        (Liberica, VersionSelector::Latest) => Winget("BellSoft.LibericaJDK.21.Full".into()),
        (Liberica, VersionSelector::Major(n)) => Winget(format!("BellSoft.LibericaJDK.{n}.Full")),
        (MicrosoftOpenjdk, VersionSelector::Latest) => Winget("Microsoft.OpenJDK.21".into()),
        (MicrosoftOpenjdk, VersionSelector::Major(n)) => Winget(format!("Microsoft.OpenJDK.{n}")),
    }
}

/// Router decision — split from execution so the routing logic is
/// exhaustively testable without spawning subprocesses.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    /// Run this route.
    RunRoute(InstallRoute),
    /// Requested distro unsupported on this OS; fall back to the
    /// tool's default platform install.
    FallbackToDefault { requested: String, reason: String },
    /// Requested distro unsupported AND `fallback = false`; hard-error.
    Refuse { requested: String, reason: String },
}

/// Pure decision function. `requested` is the raw user string
/// (whatever landed in `Tool.distribution`); the parser rejects
/// unknowns before any route is constructed, so package-name
/// interpolation only ever sees a bounded enum + a normalized version.
fn decide(requested: &str, version: &str, fallback: bool) -> Decision {
    let ver = normalize_version(version);
    match JdkDistribution::parse(requested) {
        None => {
            let reason = format!("unknown distribution '{requested}'");
            if fallback {
                Decision::FallbackToDefault {
                    requested: requested.to_string(),
                    reason,
                }
            } else {
                Decision::Refuse {
                    requested: requested.to_string(),
                    reason,
                }
            }
        }
        Some(distro) => {
            let route = resolve_route(distro, ver);
            match route {
                InstallRoute::Unsupported { reason } => {
                    if fallback {
                        Decision::FallbackToDefault {
                            requested: requested.to_string(),
                            reason,
                        }
                    } else {
                        Decision::Refuse {
                            requested: requested.to_string(),
                            reason,
                        }
                    }
                }
                other => Decision::RunRoute(other),
            }
        }
    }
}

fn resolve_route(distro: JdkDistribution, ver: VersionSelector) -> InstallRoute {
    #[cfg(target_os = "macos")]
    {
        return resolve_macos(distro, ver);
    }
    #[cfg(target_os = "linux")]
    {
        return resolve_linux(distro, ver);
    }
    #[cfg(target_os = "windows")]
    {
        return resolve_windows(distro, ver);
    }
    #[allow(unreachable_code)]
    InstallRoute::Unsupported {
        reason: format!(
            "distribution '{}' has no route on this OS",
            distro.as_slug()
        ),
    }
}

fn install_java(min_hint: &str) -> Result<(), InstallError> {
    let ovr = get_tool_override("java");
    let requested = ovr.distribution.as_deref().unwrap_or("openjdk");

    // Telemetry: `distribution` field is the parsed slug (bounded
    // enum) when parseable, else `openjdk` as the sentinel — matches
    // what the caller actually installs on fallback.
    let selected_slug = JdkDistribution::parse(requested)
        .map(|d| d.as_slug())
        .unwrap_or("openjdk");
    emit_distribution_selected(selected_slug, min_hint);

    match decide(requested, min_hint, ovr.fallback) {
        Decision::RunRoute(route) => execute_route(route),
        Decision::FallbackToDefault { requested, reason } => {
            eprintln!("warn: {reason}. Falling back to openjdk.");
            emit_distribution_fallback(&requested, true, &reason);
            JAVA.install_platform()
        }
        Decision::Refuse { requested, reason } => {
            emit_distribution_fallback(&requested, false, &reason);
            // `InstallError::Prereq` carries a `&'static str`; leak
            // the owned message so the discriminant + operator-facing
            // hint survive without inventing a new error variant.
            let msg = format!(
                "Java distribution '{requested}' is not supported on this OS and fallback = false: {reason}"
            );
            Err(InstallError::Prereq(Box::leak(msg.into_boxed_str())))
        }
    }
}

/// Emit `tool.distribution_selected` via the tracing subscriber.
/// Library code cannot reach `crate::telemetry` (bin-only module), so
/// events are emitted directly and the OTLP layer picks them up.
/// Gated on the crate-wide consent gate so `telemetry.enabled = false`
/// silences these events even when an OTLP endpoint is set for
/// unrelated reasons.
fn emit_distribution_selected(distribution: &str, version: &str) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(
        event = "tool.distribution_selected",
        tool = "java",
        distribution = %distribution,
        version = %version,
    );
}

fn emit_distribution_fallback(requested: &str, fell_back: bool, reason: &str) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::warn!(
        event = "tool.distribution_fallback",
        tool = "java",
        requested = %requested,
        fell_back = %fell_back,
        reason = %reason,
    );
}

fn execute_route(route: InstallRoute) -> Result<(), InstallError> {
    match route {
        InstallRoute::BrewFormula(pkg) => brew_install(&["install", &pkg]),
        InstallRoute::BrewCask(pkg) => brew_install(&["install", "--cask", &pkg]),
        InstallRoute::AptPkg(pkg) => apt_install(&pkg),
        InstallRoute::DnfPkg(pkg) => dnf_install(&pkg),
        InstallRoute::Winget(id) => winget_install(&id),
        InstallRoute::Choco(id) => choco_install(&id),
        InstallRoute::Unsupported { .. } => {
            // Unreachable via `decide`, which converts Unsupported
            // into Fallback / Refuse before reaching execute_route.
            // Route the panic-safe path through the "no platform
            // installer" discriminant so `is_no_platform_installer()`
            // classifies it correctly.
            Err(InstallError::Unsupported)
        }
    }
}

fn brew_install(args: &[&str]) -> Result<(), InstallError> {
    if !has("brew") {
        return Err(InstallError::Prereq(
            "Homebrew not found. Install https://brew.sh and re-run.",
        ));
    }
    run("brew", args)?;
    Ok(())
}

fn apt_install(pkg: &str) -> Result<(), InstallError> {
    use crate::tools::common::{PackageManager, PkgOps, default_use_sudo};
    let _ = PkgOps::update(PackageManager::Apt, default_use_sudo());
    PkgOps::install(PackageManager::Apt, pkg, default_use_sudo())
}

fn dnf_install(pkg: &str) -> Result<(), InstallError> {
    use crate::tools::common::{PackageManager, PkgOps, default_use_sudo};
    let _ = PkgOps::update(PackageManager::Dnf, default_use_sudo());
    PkgOps::install(PackageManager::Dnf, pkg, default_use_sudo())
}

fn winget_install(id: &str) -> Result<(), InstallError> {
    if !has("winget") {
        return Err(InstallError::Prereq(
            "winget not found. Install Windows Package Manager, then re-run.",
        ));
    }
    run("winget", &["install", "-e", "--id", id])?;
    Ok(())
}

fn choco_install(id: &str) -> Result<(), InstallError> {
    if !has("choco") {
        return Err(InstallError::Prereq(
            "chocolatey not found. Install Chocolatey, then re-run.",
        ));
    }
    run("choco", &["install", "-y", id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_registration_shape() {
        assert_eq!(JAVA.command, "java");
        let mac = JAVA.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("openjdk"));
        let win = JAVA.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Microsoft.OpenJDK.21"));
        assert!(
            JAVA.custom_install.is_some(),
            "must route through install_java"
        );
    }

    #[test]
    fn distribution_parse_known() {
        for (input, expected) in [
            ("openjdk", JdkDistribution::Openjdk),
            ("OPENJDK", JdkDistribution::Openjdk),
            ("temurin", JdkDistribution::Temurin),
            ("TEMURIN", JdkDistribution::Temurin),
            ("adoptium", JdkDistribution::Temurin),
            ("zulu", JdkDistribution::Zulu),
            ("azul", JdkDistribution::Zulu),
            ("corretto", JdkDistribution::Corretto),
            ("amazon", JdkDistribution::Corretto),
            ("liberica", JdkDistribution::Liberica),
            ("bellsoft", JdkDistribution::Liberica),
            ("microsoft-openjdk", JdkDistribution::MicrosoftOpenjdk),
            ("msopenjdk", JdkDistribution::MicrosoftOpenjdk),
            ("microsoft", JdkDistribution::MicrosoftOpenjdk),
        ] {
            assert_eq!(
                JdkDistribution::parse(input),
                Some(expected),
                "parse({input:?})"
            );
        }
        // Round-trip canonical slugs.
        for d in [
            JdkDistribution::Openjdk,
            JdkDistribution::Temurin,
            JdkDistribution::Zulu,
            JdkDistribution::Corretto,
            JdkDistribution::Liberica,
            JdkDistribution::MicrosoftOpenjdk,
        ] {
            assert_eq!(JdkDistribution::parse(d.as_slug()), Some(d));
        }
    }

    #[test]
    fn distribution_parse_unknown() {
        for bad in ["random", "foo; rm", "", "  ", "openjdk8", "temurin17"] {
            assert_eq!(JdkDistribution::parse(bad), None, "parse({bad:?})");
        }
    }

    #[test]
    fn version_normalize_latest() {
        for v in ["", "latest", "LATEST", "  ", "  latest  "] {
            assert_eq!(normalize_version(v), VersionSelector::Latest, "v={v:?}");
        }
    }

    #[test]
    fn version_normalize_major() {
        assert_eq!(normalize_version("8"), VersionSelector::Major(8));
        assert_eq!(normalize_version("17"), VersionSelector::Major(17));
        assert_eq!(normalize_version("21"), VersionSelector::Major(21));
        assert_eq!(normalize_version("99"), VersionSelector::Major(99));
    }

    #[test]
    fn version_normalize_rejects_shell_meta() {
        for bad in [
            "17; rm -rf /",
            "17 || evil",
            "$(echo pwned)",
            "17;",
            "17|17",
            "`whoami`",
            "17 17",
        ] {
            assert_eq!(
                normalize_version(bad),
                VersionSelector::Latest,
                "shell-meta should coerce to Latest: {bad:?}"
            );
        }
    }

    #[test]
    fn version_normalize_rejects_out_of_range() {
        for bad in ["0", "3", "7", "100", "999", "12345"] {
            assert_eq!(
                normalize_version(bad),
                VersionSelector::Latest,
                "out-of-range must coerce to Latest: {bad:?}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_route_macos_openjdk_latest() {
        assert_eq!(
            resolve_route(JdkDistribution::Openjdk, VersionSelector::Latest),
            InstallRoute::BrewFormula("openjdk".into())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_route_macos_openjdk_17() {
        assert_eq!(
            resolve_route(JdkDistribution::Openjdk, VersionSelector::Major(17)),
            InstallRoute::BrewFormula("openjdk@17".into())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_route_macos_temurin_17() {
        assert_eq!(
            resolve_route(JdkDistribution::Temurin, VersionSelector::Major(17)),
            InstallRoute::BrewCask("temurin@17".into())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_route_macos_zulu_17() {
        assert_eq!(
            resolve_route(JdkDistribution::Zulu, VersionSelector::Major(17)),
            InstallRoute::BrewCask("zulu@17".into())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_route_macos_corretto_17() {
        assert_eq!(
            resolve_route(JdkDistribution::Corretto, VersionSelector::Major(17)),
            InstallRoute::BrewCask("corretto@17".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolve_route_linux_temurin_17_unsupported() {
        match resolve_route(JdkDistribution::Temurin, VersionSelector::Major(17)) {
            InstallRoute::Unsupported { .. } => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn resolve_route_windows_temurin_17() {
        assert_eq!(
            resolve_route(JdkDistribution::Temurin, VersionSelector::Major(17)),
            InstallRoute::Winget("EclipseAdoptium.Temurin.17.JDK".into())
        );
    }

    #[test]
    fn decide_unknown_distribution_falls_back_when_allowed() {
        match decide("bogus-jdk", "17", true) {
            Decision::FallbackToDefault { requested, reason } => {
                assert_eq!(requested, "bogus-jdk");
                assert!(reason.contains("unknown distribution"));
            }
            other => panic!("expected FallbackToDefault, got {other:?}"),
        }
    }

    #[test]
    fn decide_unknown_distribution_refuses_when_fallback_false() {
        match decide("bogus-jdk", "17", false) {
            Decision::Refuse { requested, reason } => {
                assert_eq!(requested, "bogus-jdk");
                assert!(reason.contains("unknown distribution"));
            }
            other => panic!("expected Refuse, got {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decide_linux_temurin_falls_back_when_allowed() {
        match decide("temurin", "17", true) {
            Decision::FallbackToDefault { requested, .. } => {
                assert_eq!(requested, "temurin");
            }
            other => panic!("expected FallbackToDefault, got {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decide_linux_temurin_refuses_when_fallback_false() {
        match decide("temurin", "17", false) {
            Decision::Refuse { requested, .. } => {
                assert_eq!(requested, "temurin");
            }
            other => panic!("expected Refuse, got {other:?}"),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn decide_macos_temurin_runs_route() {
        match decide("temurin", "17", true) {
            Decision::RunRoute(InstallRoute::BrewCask(pkg)) => {
                assert_eq!(pkg, "temurin@17");
            }
            other => panic!("expected RunRoute(BrewCask), got {other:?}"),
        }
    }

    #[test]
    fn decide_shell_meta_version_is_safe() {
        // Version-injection attempt still hits the router — but
        // `normalize_version` coerces it to Latest so no shell
        // characters reach a package-name interpolation.
        match decide("openjdk", "17; rm -rf /", true) {
            Decision::RunRoute(route) => {
                let pkg_ok = match &route {
                    InstallRoute::BrewFormula(s)
                    | InstallRoute::BrewCask(s)
                    | InstallRoute::AptPkg(s)
                    | InstallRoute::DnfPkg(s)
                    | InstallRoute::Winget(s)
                    | InstallRoute::Choco(s) => {
                        !s.contains(';') && !s.contains('|') && !s.contains('`')
                    }
                    InstallRoute::Unsupported { .. } => true,
                };
                assert!(pkg_ok, "shell metachars leaked into route: {route:?}");
            }
            // Some OSes (linux with non-apt/dnf PM) return Unsupported
            // for openjdk-latest — also safe.
            Decision::FallbackToDefault { .. } => {}
            Decision::Refuse { .. } => panic!("openjdk with fallback=true must not refuse"),
        }
    }
}
