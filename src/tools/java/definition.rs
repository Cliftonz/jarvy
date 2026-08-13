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
#[cfg(target_os = "linux")]
use crate::tools::common::{PackageManager, require};

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
    AptPkgWithRepo(&'static VendorRepo, String),
    DnfPkg(String),
    DnfPkgWithRepo(&'static VendorRepo, String),
    Winget(String),
    Choco(String),
    Unsupported { reason: String },
}

/// Vendor-provided apt or dnf repository description. All fields are
/// `&'static str` and every value below is defined as a `const` — no
/// user input reaches the shell-escaped body, so shell-metacharacter
/// injection is bounded to the `%CODENAME%` substitution which is
/// validated separately by `is_valid_debian_codename`.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct VendorRepo {
    /// Human-readable vendor label used in log lines.
    vendor: &'static str,
    /// Absolute URL to the vendor's GPG key (must be HTTPS).
    key_url: &'static str,
    /// Path where the dearmored key is written (must live under
    /// `/etc/apt/keyrings` or `/usr/share/keyrings` for apt, or
    /// `/etc/pki/rpm-gpg` for dnf).
    key_path: &'static str,
    /// Path where the `sources.list.d/*.list` (apt) or
    /// `yum.repos.d/*.repo` (dnf) file is written.
    list_path: &'static str,
    /// Body of the `.list` / `.repo` file. `%CODENAME%` is substituted
    /// with `lsb_release -cs` output at bootstrap time (apt only). dnf
    /// bodies do not carry substitutions.
    body: &'static str,
}

// ---- Adoptium Temurin ----
#[cfg(target_os = "linux")]
const TEMURIN_APT: VendorRepo = VendorRepo {
    vendor: "Adoptium Temurin",
    key_url: "https://packages.adoptium.net/artifactory/api/gpg/key/public",
    key_path: "/etc/apt/keyrings/adoptium.gpg",
    list_path: "/etc/apt/sources.list.d/adoptium.list",
    body: "deb [signed-by=/etc/apt/keyrings/adoptium.gpg] https://packages.adoptium.net/artifactory/deb %CODENAME% main\n",
};

#[cfg(target_os = "linux")]
const TEMURIN_DNF: VendorRepo = VendorRepo {
    vendor: "Adoptium Temurin",
    key_url: "https://packages.adoptium.net/artifactory/api/gpg/key/public",
    key_path: "/etc/pki/rpm-gpg/adoptium.gpg",
    list_path: "/etc/yum.repos.d/adoptium.repo",
    body: "[Adoptium]\nname=Adoptium\nbaseurl=https://packages.adoptium.net/artifactory/rpm/rockylinux/$releasever/$basearch\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/adoptium.gpg\n",
};

// ---- Azul Zulu ----
#[cfg(target_os = "linux")]
const ZULU_APT: VendorRepo = VendorRepo {
    vendor: "Azul Zulu",
    key_url: "https://repos.azul.com/azul-repo.key",
    key_path: "/usr/share/keyrings/azul.gpg",
    list_path: "/etc/apt/sources.list.d/zulu.list",
    body: "deb [signed-by=/usr/share/keyrings/azul.gpg] https://repos.azul.com/zulu/deb stable main\n",
};

#[cfg(target_os = "linux")]
const ZULU_DNF: VendorRepo = VendorRepo {
    vendor: "Azul Zulu",
    key_url: "https://repos.azul.com/azul-repo.key",
    key_path: "/etc/pki/rpm-gpg/azul.gpg",
    list_path: "/etc/yum.repos.d/zulu.repo",
    body: "[zulu]\nname=zulu\nbaseurl=https://repos.azul.com/zulu/rpm\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/azul.gpg\n",
};

// ---- Amazon Corretto ----
#[cfg(target_os = "linux")]
const CORRETTO_APT: VendorRepo = VendorRepo {
    vendor: "Amazon Corretto",
    key_url: "https://apt.corretto.aws/corretto.key",
    key_path: "/usr/share/keyrings/corretto.gpg",
    list_path: "/etc/apt/sources.list.d/corretto.list",
    body: "deb [signed-by=/usr/share/keyrings/corretto.gpg] https://apt.corretto.aws stable main\n",
};

#[cfg(target_os = "linux")]
const CORRETTO_DNF: VendorRepo = VendorRepo {
    vendor: "Amazon Corretto",
    key_url: "https://yum.corretto.aws/corretto.key",
    key_path: "/etc/pki/rpm-gpg/corretto.gpg",
    list_path: "/etc/yum.repos.d/corretto.repo",
    body: "[corretto]\nname=Amazon Corretto\nbaseurl=https://yum.corretto.aws\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/corretto.gpg\n",
};

// ---- BellSoft Liberica ----
#[cfg(target_os = "linux")]
const LIBERICA_APT: VendorRepo = VendorRepo {
    vendor: "BellSoft Liberica",
    key_url: "https://download.bell-sw.com/pki/GPG-KEY-bellsoft",
    key_path: "/etc/apt/keyrings/bellsoft.gpg",
    list_path: "/etc/apt/sources.list.d/bellsoft.list",
    body: "deb [signed-by=/etc/apt/keyrings/bellsoft.gpg] https://apt.bell-sw.com/ stable main\n",
};

#[cfg(target_os = "linux")]
const LIBERICA_DNF: VendorRepo = VendorRepo {
    vendor: "BellSoft Liberica",
    key_url: "https://download.bell-sw.com/pki/GPG-KEY-bellsoft",
    key_path: "/etc/pki/rpm-gpg/bellsoft.gpg",
    list_path: "/etc/yum.repos.d/bellsoft.repo",
    body: "[bellsoft]\nname=BellSoft Repository\nbaseurl=https://yum.bell-sw.com\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/bellsoft.gpg\n",
};

// ---- Microsoft OpenJDK (dnf only; apt requires Ubuntu major-version
// detection that is not modeled in v1, so apt stays Unsupported for MS.)
#[cfg(target_os = "linux")]
const MSOPENJDK_DNF: VendorRepo = VendorRepo {
    vendor: "Microsoft OpenJDK",
    key_url: "https://packages.microsoft.com/keys/microsoft.asc",
    key_path: "/etc/pki/rpm-gpg/microsoft.gpg",
    list_path: "/etc/yum.repos.d/microsoft-prod.repo",
    body: "[packages-microsoft-com-prod]\nname=packages-microsoft-com-prod\nbaseurl=https://packages.microsoft.com/rhel/9/prod/\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/microsoft.gpg\n",
};

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
    resolve_linux_inner(crate::tools::common::detect_linux_pm(), distro, ver)
}

/// Pure PM-aware routing — extracted from `resolve_linux` so tests can
/// exercise every (PM, distro, version) triple without depending on the
/// host's actual package manager. Every version-bearing package name
/// interpolates ONLY the `Major(n)` integer from the closed
/// `VersionSelector` enum, so no user-controlled string reaches the
/// package identifier.
#[cfg(target_os = "linux")]
fn resolve_linux_inner(
    pm: Option<PackageManager>,
    distro: JdkDistribution,
    ver: VersionSelector,
) -> InstallRoute {
    use InstallRoute::*;
    use JdkDistribution::*;
    // Default Java major version used when the user asks for "latest"
    // on a vendor package that requires an explicit major in the
    // package name. 21 = current LTS as of the write of this table.
    const DEFAULT_MAJOR: u32 = 21;
    let n = match ver {
        VersionSelector::Latest => DEFAULT_MAJOR,
        VersionSelector::Major(n) => n,
    };
    match (distro, pm) {
        // openjdk retains its distro-native paths.
        (Openjdk, Some(PackageManager::Apt)) => match ver {
            VersionSelector::Latest => AptPkg("default-jdk".into()),
            VersionSelector::Major(n) => AptPkg(format!("openjdk-{n}-jdk")),
        },
        (Openjdk, Some(PackageManager::Dnf)) => match ver {
            VersionSelector::Latest => DnfPkg("java-latest-openjdk-devel".into()),
            VersionSelector::Major(n) => DnfPkg(format!("java-{n}-openjdk-devel")),
        },
        (Openjdk, other_pm) => Unsupported {
            reason: format!(
                "openjdk distribution not modeled for {other_pm:?} yet - falling back to jarvy default"
            ),
        },

        // Temurin
        (Temurin, Some(PackageManager::Apt)) => {
            AptPkgWithRepo(&TEMURIN_APT, format!("temurin-{n}-jdk"))
        }
        (Temurin, Some(PackageManager::Dnf)) => {
            DnfPkgWithRepo(&TEMURIN_DNF, format!("temurin-{n}-jdk"))
        }

        // Zulu
        (Zulu, Some(PackageManager::Apt)) => {
            AptPkgWithRepo(&ZULU_APT, format!("zulu{n}-jdk"))
        }
        (Zulu, Some(PackageManager::Dnf)) => {
            DnfPkgWithRepo(&ZULU_DNF, format!("zulu{n}-jdk"))
        }

        // Corretto
        (Corretto, Some(PackageManager::Apt)) => {
            AptPkgWithRepo(&CORRETTO_APT, format!("java-{n}-amazon-corretto-jdk"))
        }
        (Corretto, Some(PackageManager::Dnf)) => {
            DnfPkgWithRepo(&CORRETTO_DNF, format!("java-{n}-amazon-corretto-devel"))
        }

        // Liberica
        (Liberica, Some(PackageManager::Apt)) => {
            AptPkgWithRepo(&LIBERICA_APT, format!("bellsoft-java{n}-full"))
        }
        (Liberica, Some(PackageManager::Dnf)) => {
            DnfPkgWithRepo(&LIBERICA_DNF, format!("bellsoft-java{n}-full"))
        }

        // Microsoft OpenJDK — apt intentionally unsupported until
        // Ubuntu major-version detection is modeled; dnf works.
        (MicrosoftOpenjdk, Some(PackageManager::Apt)) => Unsupported {
            reason: "microsoft-openjdk apt bootstrap requires Ubuntu 20.04/22.04/24.04 detection; not yet automated - install manually per https://learn.microsoft.com/en-us/java/openjdk/install".into(),
        },
        (MicrosoftOpenjdk, Some(PackageManager::Dnf)) => {
            DnfPkgWithRepo(&MSOPENJDK_DNF, format!("msopenjdk-{n}"))
        }

        // pacman / apk / zypper / yum / other PMs: no first-party
        // vendor packaging modeled. Caller falls back to openjdk (or
        // refuses per `fallback = false`).
        (other_distro, other_pm) => Unsupported {
            reason: format!(
                "distribution '{}' on Linux with {:?} package manager has no first-party vendor repo modeled",
                other_distro.as_slug(),
                other_pm
            ),
        },
    }
}

/// Accepts only lowercase alnum Debian/Ubuntu codenames (`jammy`,
/// `noble`, `bookworm`, …). Anything containing shell metacharacters,
/// whitespace, uppercase, hyphens, dots, or non-ASCII is refused. The
/// codename is interpolated into a shell command run under sudo, so
/// this is a security boundary.
#[cfg(target_os = "linux")]
fn is_valid_debian_codename(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Idempotency: skip bootstrap if the sources.list.d/yum.repos.d file
/// already exists AND was modified within the last 30 days. Cheap
/// heuristic against re-fetching on every `jarvy setup`.
#[cfg(target_os = "linux")]
fn repo_recently_written(list_path: &str) -> bool {
    use std::time::{Duration, SystemTime};
    let Ok(md) = std::fs::metadata(list_path) else {
        return false;
    };
    let Ok(mtime) = md.modified() else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(mtime) else {
        return false;
    };
    age < Duration::from_secs(30 * 24 * 60 * 60)
}

/// Fetch the vendor GPG key over HTTPS + dearmor it into `key_path`
/// under sudo. Fails with `Prereq` if curl / gpg / lsb_release are
/// missing — never auto-installs them silently.
#[cfg(target_os = "linux")]
fn bootstrap_apt_repo(repo: &VendorRepo) -> Result<(), InstallError> {
    use crate::tools::common::{default_use_sudo, run, run_maybe_sudo};

    if repo_recently_written(repo.list_path) {
        return Ok(());
    }

    require("curl", "curl is required to fetch the vendor GPG key")?;
    require("gpg", "gpg is required to dearmor the vendor GPG key")?;
    require(
        "lsb_release",
        "lsb_release not found (install lsb-release) - required to detect the Debian/Ubuntu codename",
    )?;

    let use_sudo = default_use_sudo().unwrap_or(true);

    // Resolve codename before doing any writes.
    let codename_out = run("lsb_release", &["-cs"])?;
    let codename_raw = String::from_utf8_lossy(&codename_out.stdout);
    let codename = codename_raw.trim();
    if !is_valid_debian_codename(codename) {
        return Err(InstallError::Prereq(
            "lsb_release returned a codename with unexpected characters; refusing to bootstrap vendor repo",
        ));
    }

    // Ensure key parent dir exists (e.g. /etc/apt/keyrings on newer
    // Debian derivatives).
    if let Some(parent) = std::path::Path::new(repo.key_path).parent() {
        let parent_str = parent.to_string_lossy();
        run_maybe_sudo(use_sudo, "mkdir", &["-p", &parent_str])?;
    }

    // Fetch the key to a tmp file, then dearmor via a shell pipeline
    // that itself runs under sudo so the resulting file lands with
    // root ownership at `key_path`. The pipeline body uses ONLY
    // `&'static str` inputs from `VendorRepo` and the validated
    // codename — no user string reaches the shell.
    let dearmor_cmd = format!(
        "set -euo pipefail; curl -fsSL {} | gpg --dearmor --yes -o {}",
        shell_single_quote(repo.key_url),
        shell_single_quote(repo.key_path)
    );
    run_maybe_sudo(use_sudo, "sh", &["-c", &dearmor_cmd])?;

    // Write the sources.list file via `sudo tee`. Body is a static
    // const with one `%CODENAME%` substitution; the substitution has
    // been validated above.
    let body = repo.body.replace("%CODENAME%", codename);
    let write_cmd = format!(
        "printf '%s' {} | tee {} > /dev/null",
        shell_single_quote(&body),
        shell_single_quote(repo.list_path)
    );
    run_maybe_sudo(use_sudo, "sh", &["-c", &write_cmd])?;

    // Refresh apt indexes so the newly-added repo is visible to the
    // subsequent `PkgOps::install` call.
    let apt = if has("apt") { "apt" } else { "apt-get" };
    run_maybe_sudo(use_sudo, apt, &["update"])?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn bootstrap_dnf_repo(repo: &VendorRepo) -> Result<(), InstallError> {
    use crate::tools::common::{default_use_sudo, run_maybe_sudo};

    if repo_recently_written(repo.list_path) {
        return Ok(());
    }

    require("curl", "curl is required to fetch the vendor GPG key")?;

    let use_sudo = default_use_sudo().unwrap_or(true);

    if let Some(parent) = std::path::Path::new(repo.key_path).parent() {
        let parent_str = parent.to_string_lossy();
        run_maybe_sudo(use_sudo, "mkdir", &["-p", &parent_str])?;
    }

    // dnf accepts ASCII-armored keys directly (gpgkey=file://…), so
    // no dearmor step is required — just fetch to key_path under sudo.
    let fetch_cmd = format!(
        "set -euo pipefail; curl -fsSL {} -o {}",
        shell_single_quote(repo.key_url),
        shell_single_quote(repo.key_path)
    );
    run_maybe_sudo(use_sudo, "sh", &["-c", &fetch_cmd])?;

    let write_cmd = format!(
        "printf '%s' {} | tee {} > /dev/null",
        shell_single_quote(repo.body),
        shell_single_quote(repo.list_path)
    );
    run_maybe_sudo(use_sudo, "sh", &["-c", &write_cmd])?;

    Ok(())
}

/// Wraps a string in POSIX single quotes, escaping any embedded `'`
/// so the value is safe inside a `sh -c` pipeline. Repo bodies + URLs
/// are `&'static str` constants and the codename is validated, but
/// this keeps every string that reaches `sh` uniformly quoted.
#[cfg(target_os = "linux")]
fn shell_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
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
        InstallRoute::AptPkgWithRepo(repo, pkg) => {
            #[cfg(target_os = "linux")]
            {
                bootstrap_apt_repo(repo)?;
                return apt_install(&pkg);
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (repo, pkg);
                Err(InstallError::Unsupported)
            }
        }
        InstallRoute::DnfPkg(pkg) => dnf_install(&pkg),
        InstallRoute::DnfPkgWithRepo(repo, pkg) => {
            #[cfg(target_os = "linux")]
            {
                bootstrap_dnf_repo(repo)?;
                return dnf_install(&pkg);
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (repo, pkg);
                Err(InstallError::Unsupported)
            }
        }
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
    fn linux_temurin_apt_route_v1() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Apt),
                JdkDistribution::Temurin,
                VersionSelector::Major(17),
            ),
            InstallRoute::AptPkgWithRepo(&TEMURIN_APT, "temurin-17-jdk".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_temurin_dnf_route_v1() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Dnf),
                JdkDistribution::Temurin,
                VersionSelector::Major(21),
            ),
            InstallRoute::DnfPkgWithRepo(&TEMURIN_DNF, "temurin-21-jdk".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_zulu_apt_route_v1() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Apt),
                JdkDistribution::Zulu,
                VersionSelector::Major(17),
            ),
            InstallRoute::AptPkgWithRepo(&ZULU_APT, "zulu17-jdk".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_zulu_dnf_route_v1() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Dnf),
                JdkDistribution::Zulu,
                VersionSelector::Major(17),
            ),
            InstallRoute::DnfPkgWithRepo(&ZULU_DNF, "zulu17-jdk".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_corretto_apt_route_v1() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Apt),
                JdkDistribution::Corretto,
                VersionSelector::Major(17),
            ),
            InstallRoute::AptPkgWithRepo(&CORRETTO_APT, "java-17-amazon-corretto-jdk".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_corretto_dnf_route_v1() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Dnf),
                JdkDistribution::Corretto,
                VersionSelector::Major(17),
            ),
            InstallRoute::DnfPkgWithRepo(&CORRETTO_DNF, "java-17-amazon-corretto-devel".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_liberica_apt_route_v1() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Apt),
                JdkDistribution::Liberica,
                VersionSelector::Major(17),
            ),
            InstallRoute::AptPkgWithRepo(&LIBERICA_APT, "bellsoft-java17-full".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_liberica_dnf_route_v1() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Dnf),
                JdkDistribution::Liberica,
                VersionSelector::Major(17),
            ),
            InstallRoute::DnfPkgWithRepo(&LIBERICA_DNF, "bellsoft-java17-full".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn microsoft_openjdk_apt_still_unsupported() {
        match resolve_linux_inner(
            Some(PackageManager::Apt),
            JdkDistribution::MicrosoftOpenjdk,
            VersionSelector::Major(21),
        ) {
            InstallRoute::Unsupported { reason } => {
                assert!(
                    reason.contains("microsoft-openjdk"),
                    "reason should mention microsoft-openjdk: {reason}"
                );
            }
            other => panic!("expected Unsupported for msopenjdk apt, got {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn microsoft_openjdk_dnf_bootstraps() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Dnf),
                JdkDistribution::MicrosoftOpenjdk,
                VersionSelector::Major(21),
            ),
            InstallRoute::DnfPkgWithRepo(&MSOPENJDK_DNF, "msopenjdk-21".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_temurin_latest_uses_default_major() {
        assert_eq!(
            resolve_linux_inner(
                Some(PackageManager::Apt),
                JdkDistribution::Temurin,
                VersionSelector::Latest,
            ),
            InstallRoute::AptPkgWithRepo(&TEMURIN_APT, "temurin-21-jdk".into())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_pacman_vendor_distros_unsupported() {
        for distro in [
            JdkDistribution::Temurin,
            JdkDistribution::Zulu,
            JdkDistribution::Corretto,
            JdkDistribution::Liberica,
        ] {
            match resolve_linux_inner(
                Some(PackageManager::Pacman),
                distro,
                VersionSelector::Major(17),
            ) {
                InstallRoute::Unsupported { .. } => {}
                other => panic!("expected Unsupported for {distro:?} on pacman, got {other:?}"),
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn codename_validate_rejects_shell_meta() {
        for bad in [
            "",
            "Jammy",       // uppercase
            "jammy; rm",   // shell metachar
            "$( whoami )", // command substitution
            "jammy noble", // whitespace
            "jammy-lts",   // hyphen
            "jammy.1",     // dot
            "jammy`",      // backtick
            "jammy|noble", // pipe
            "jammy\n",     // newline
            "jámmy",       // non-ASCII
        ] {
            assert!(
                !is_valid_debian_codename(bad),
                "codename {bad:?} should be rejected"
            );
        }
        for good in ["jammy", "noble", "bookworm", "trixie", "focal", "bullseye"] {
            assert!(
                is_valid_debian_codename(good),
                "codename {good:?} should be accepted"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shell_single_quote_escapes_embedded_quote() {
        assert_eq!(shell_single_quote("hi"), "'hi'");
        assert_eq!(shell_single_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_single_quote(""), "''");
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
    fn decide_linux_temurin_result_depends_on_host_pm() {
        // On apt/dnf hosts, temurin now bootstraps the vendor repo
        // (RunRoute). On pacman/apk/… hosts, it falls back (or refuses
        // when fallback = false). Either outcome is correct per the
        // routing table; the test just asserts the requested slug is
        // preserved through the decision.
        match decide("temurin", "17", true) {
            Decision::RunRoute(_) | Decision::FallbackToDefault { .. } => {}
            other => panic!("expected RunRoute or FallbackToDefault, got {other:?}"),
        }
        match decide("temurin", "17", false) {
            Decision::RunRoute(_) => {}
            Decision::Refuse { requested, .. } => {
                assert_eq!(requested, "temurin");
            }
            other => panic!("expected RunRoute or Refuse, got {other:?}"),
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
                    | InstallRoute::AptPkgWithRepo(_, s)
                    | InstallRoute::DnfPkg(s)
                    | InstallRoute::DnfPkgWithRepo(_, s)
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
