//! java - Java Development Kit
//!
//! Distribution-aware install path: honors `Tool.distribution`
//! (`openjdk` / `temurin` / `zulu` / `corretto` / `liberica` /
//! `microsoft-openjdk`) and `Tool.fallback` threaded through the
//! `InstallContext` argument passed by `setup_cmd`. On unsupported
//! (distribution, OS) pairs the router warns and falls back to openjdk
//! unless `fallback = false`, in which case it hard-errors via
//! `InstallError::Prereq`.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, has, run};
#[cfg(target_os = "linux")]
use crate::tools::common::{PackageManager, require};
#[cfg(target_os = "linux")]
use std::sync::{Mutex, OnceLock};

/// Default Java major version used when the user asks for "latest"
/// on a vendor package that requires an explicit major in the
/// package name. 21 = current LTS as of the write of this table.
///
/// `allow(dead_code)`: only referenced from cfg-conditional linux/windows
/// routers and tests; the macos host build would otherwise warn.
#[allow(dead_code)]
const DEFAULT_MAJOR: u32 = 21;

/// Per-process memo: has `apt-get update` already been run this
/// invocation? `bootstrap_apt_repo` flips this to `true` after its
/// own update, and `apt_install` skips the redundant update if the
/// flag is set. Not persisted across process invocations; jarvy
/// setup is one process per run.
#[cfg(target_os = "linux")]
static APT_UPDATED: OnceLock<Mutex<bool>> = OnceLock::new();
#[cfg(target_os = "linux")]
static DNF_UPDATED: OnceLock<Mutex<bool>> = OnceLock::new();

#[cfg(target_os = "linux")]
fn mark_pm_updated(cell: &OnceLock<Mutex<bool>>) {
    let m = cell.get_or_init(|| Mutex::new(false));
    if let Ok(mut g) = m.lock() {
        *g = true;
    }
}

#[cfg(target_os = "linux")]
fn pm_already_updated(cell: &OnceLock<Mutex<bool>>) -> bool {
    cell.get()
        .and_then(|m| m.lock().ok().map(|g| *g))
        .unwrap_or(false)
}

define_tool!(JAVA, {
    command: "java",
    macos: { brew: "openjdk" },
    linux: { apt: "default-jdk", dnf: "java-latest-openjdk", pacman: "jdk-openjdk", apk: "openjdk21" },
    // KEEP IN SYNC WITH DEFAULT_MAJOR above (macros can't consume `const`s).
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
    AptPkg {
        pkg: String,
        repo: Option<&'static VendorRepo>,
    },
    DnfPkg {
        pkg: String,
        repo: Option<&'static VendorRepo>,
    },
    Winget(String),
    Choco(String),
    Unsupported {
        reason_kind: &'static str,
        reason: String,
    },
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
    /// Expected 40-char uppercase-hex GPG key fingerprint. Compared
    /// (case-insensitive) against the fingerprint reported by
    /// `gpg --show-keys --with-fingerprint --with-colons` after the
    /// key lands on disk. Sentinel `"UNVERIFIED"` disables the check
    /// (only for vendors whose current fingerprint I couldn't verify
    /// from official docs at write time — see TODO(security)).
    expected_fingerprint: &'static str,
}

// ---- Adoptium Temurin ----
#[cfg(target_os = "linux")]
const TEMURIN_APT: VendorRepo = VendorRepo {
    vendor: "Adoptium Temurin",
    key_url: "https://packages.adoptium.net/artifactory/api/gpg/key/public",
    key_path: "/etc/apt/keyrings/adoptium.gpg",
    list_path: "/etc/apt/sources.list.d/adoptium.list",
    body: "deb [signed-by=/etc/apt/keyrings/adoptium.gpg] https://packages.adoptium.net/artifactory/deb %CODENAME% main\n",
    // Verified 2026-08-13 by fetching
    // packages.adoptium.net/artifactory/api/gpg/key/public and running
    // `gpg --show-keys --with-fingerprint --with-colons`. Primary key
    // matches; secondary subkey 45B0F08FF61F64C1C1F61E173F4A517504A9CD61
    // is signed by this primary and rotates independently.
    expected_fingerprint: "3B04D753C9050D9A5D343F39843C48A565F8F04B",
};

#[cfg(target_os = "linux")]
const TEMURIN_DNF: VendorRepo = VendorRepo {
    vendor: "Adoptium Temurin",
    key_url: "https://packages.adoptium.net/artifactory/api/gpg/key/public",
    key_path: "/etc/pki/rpm-gpg/adoptium.gpg",
    list_path: "/etc/yum.repos.d/adoptium.repo",
    body: "[Adoptium]\nname=Adoptium\nbaseurl=https://packages.adoptium.net/artifactory/rpm/rockylinux/$releasever/$basearch\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/adoptium.gpg\n",
    // Same key as the apt variant — verified 2026-08-13.
    expected_fingerprint: "3B04D753C9050D9A5D343F39843C48A565F8F04B",
};

// ---- Azul Zulu ----
#[cfg(target_os = "linux")]
const ZULU_APT: VendorRepo = VendorRepo {
    vendor: "Azul Zulu",
    key_url: "https://repos.azul.com/azul-repo.key",
    key_path: "/usr/share/keyrings/azul.gpg",
    list_path: "/etc/apt/sources.list.d/zulu.list",
    body: "deb [signed-by=/usr/share/keyrings/azul.gpg] https://repos.azul.com/zulu/deb stable main\n",
    // Verified 2026-08-13 by fetching repos.azul.com/azul-repo.key and
    // running `gpg --show-keys --with-fingerprint --with-colons`.
    // Matches the short-id 0xB1998361219BD9C9 published on docs.azul.com.
    expected_fingerprint: "27BC0C8CB3D81623F59BDADCB1998361219BD9C9",
};

#[cfg(target_os = "linux")]
const ZULU_DNF: VendorRepo = VendorRepo {
    vendor: "Azul Zulu",
    key_url: "https://repos.azul.com/azul-repo.key",
    key_path: "/etc/pki/rpm-gpg/azul.gpg",
    list_path: "/etc/yum.repos.d/zulu.repo",
    body: "[zulu]\nname=zulu\nbaseurl=https://repos.azul.com/zulu/rpm\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/azul.gpg\n",
    // Same key as the apt variant — verified 2026-08-13.
    expected_fingerprint: "27BC0C8CB3D81623F59BDADCB1998361219BD9C9",
};

// ---- Amazon Corretto ----
#[cfg(target_os = "linux")]
const CORRETTO_APT: VendorRepo = VendorRepo {
    vendor: "Amazon Corretto",
    key_url: "https://apt.corretto.aws/corretto.key",
    key_path: "/usr/share/keyrings/corretto.gpg",
    list_path: "/etc/apt/sources.list.d/corretto.list",
    body: "deb [signed-by=/usr/share/keyrings/corretto.gpg] https://apt.corretto.aws stable main\n",
    // Verified 2026-08-13 by fetching apt.corretto.aws/corretto.key and
    // running `gpg --show-keys --with-fingerprint --with-colons`.
    // Matches the long-id A122542AB04F24E3 published in the AWS Corretto
    // install guide's "NO_PUBKEY" recovery hint.
    expected_fingerprint: "6DC3636DAE534049C8B94623A122542AB04F24E3",
};

#[cfg(target_os = "linux")]
const CORRETTO_DNF: VendorRepo = VendorRepo {
    vendor: "Amazon Corretto",
    key_url: "https://yum.corretto.aws/corretto.key",
    key_path: "/etc/pki/rpm-gpg/corretto.gpg",
    list_path: "/etc/yum.repos.d/corretto.repo",
    body: "[corretto]\nname=Amazon Corretto\nbaseurl=https://yum.corretto.aws\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/corretto.gpg\n",
    // Same key as the apt variant — verified 2026-08-13.
    expected_fingerprint: "6DC3636DAE534049C8B94623A122542AB04F24E3",
};

// ---- BellSoft Liberica ----
#[cfg(target_os = "linux")]
const LIBERICA_APT: VendorRepo = VendorRepo {
    vendor: "BellSoft Liberica",
    key_url: "https://download.bell-sw.com/pki/GPG-KEY-bellsoft",
    key_path: "/etc/apt/keyrings/bellsoft.gpg",
    list_path: "/etc/apt/sources.list.d/bellsoft.list",
    body: "deb [signed-by=/etc/apt/keyrings/bellsoft.gpg] https://apt.bell-sw.com/ stable main\n",
    // Verified 2026-08-13 by fetching
    // download.bell-sw.com/pki/GPG-KEY-bellsoft and running
    // `gpg --show-keys --with-fingerprint --with-colons`.
    expected_fingerprint: "99A5C88E3C5B1FA8B05A19D332E9750179FCEA62",
};

#[cfg(target_os = "linux")]
const LIBERICA_DNF: VendorRepo = VendorRepo {
    vendor: "BellSoft Liberica",
    key_url: "https://download.bell-sw.com/pki/GPG-KEY-bellsoft",
    key_path: "/etc/pki/rpm-gpg/bellsoft.gpg",
    list_path: "/etc/yum.repos.d/bellsoft.repo",
    body: "[bellsoft]\nname=BellSoft Repository\nbaseurl=https://yum.bell-sw.com\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/bellsoft.gpg\n",
    // Same key as the apt variant — verified 2026-08-13.
    expected_fingerprint: "99A5C88E3C5B1FA8B05A19D332E9750179FCEA62",
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
    // Verified 2026-08-13 by fetching packages.microsoft.com/keys/microsoft.asc
    // and running `gpg --show-keys --with-fingerprint --with-colons`.
    expected_fingerprint: "BC528686B50D79E339D3721CEB3E94ADBE1229CF",
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
    let n = match ver {
        VersionSelector::Latest => DEFAULT_MAJOR,
        VersionSelector::Major(n) => n,
    };
    match (distro, pm) {
        // openjdk retains its distro-native paths.
        (Openjdk, Some(PackageManager::Apt)) => match ver {
            VersionSelector::Latest => AptPkg {
                pkg: "default-jdk".into(),
                repo: None,
            },
            VersionSelector::Major(n) => AptPkg {
                pkg: format!("openjdk-{n}-jdk"),
                repo: None,
            },
        },
        (Openjdk, Some(PackageManager::Dnf)) => match ver {
            VersionSelector::Latest => DnfPkg {
                pkg: "java-latest-openjdk-devel".into(),
                repo: None,
            },
            VersionSelector::Major(n) => DnfPkg {
                pkg: format!("java-{n}-openjdk-devel"),
                repo: None,
            },
        },
        (Openjdk, other_pm) => Unsupported {
            reason_kind: "unsupported_pm",
            reason: format!(
                "openjdk distribution not modeled for {other_pm:?} yet - falling back to jarvy default"
            ),
        },

        // Temurin
        (Temurin, Some(PackageManager::Apt)) => AptPkg {
            pkg: format!("temurin-{n}-jdk"),
            repo: Some(&TEMURIN_APT),
        },
        (Temurin, Some(PackageManager::Dnf)) => DnfPkg {
            pkg: format!("temurin-{n}-jdk"),
            repo: Some(&TEMURIN_DNF),
        },

        // Zulu
        (Zulu, Some(PackageManager::Apt)) => AptPkg {
            pkg: format!("zulu{n}-jdk"),
            repo: Some(&ZULU_APT),
        },
        (Zulu, Some(PackageManager::Dnf)) => DnfPkg {
            pkg: format!("zulu{n}-jdk"),
            repo: Some(&ZULU_DNF),
        },

        // Corretto
        (Corretto, Some(PackageManager::Apt)) => AptPkg {
            pkg: format!("java-{n}-amazon-corretto-jdk"),
            repo: Some(&CORRETTO_APT),
        },
        (Corretto, Some(PackageManager::Dnf)) => DnfPkg {
            pkg: format!("java-{n}-amazon-corretto-devel"),
            repo: Some(&CORRETTO_DNF),
        },

        // Liberica
        (Liberica, Some(PackageManager::Apt)) => AptPkg {
            pkg: format!("bellsoft-java{n}-full"),
            repo: Some(&LIBERICA_APT),
        },
        (Liberica, Some(PackageManager::Dnf)) => DnfPkg {
            pkg: format!("bellsoft-java{n}-full"),
            repo: Some(&LIBERICA_DNF),
        },

        // Microsoft OpenJDK — apt intentionally unsupported until
        // Ubuntu major-version detection is modeled; dnf works.
        (MicrosoftOpenjdk, Some(PackageManager::Apt)) => Unsupported {
            reason_kind: "msopenjdk_apt_manual",
            reason: "microsoft-openjdk apt bootstrap requires Ubuntu 20.04/22.04/24.04 detection; not yet automated - install manually per https://learn.microsoft.com/en-us/java/openjdk/install".into(),
        },
        (MicrosoftOpenjdk, Some(PackageManager::Dnf)) => DnfPkg {
            pkg: format!("msopenjdk-{n}"),
            repo: Some(&MSOPENJDK_DNF),
        },

        // pacman / apk / zypper / yum / other PMs: no first-party
        // vendor packaging modeled. Caller falls back to openjdk (or
        // refuses per `fallback = false`).
        (other_distro, other_pm) => Unsupported {
            reason_kind: "unsupported_pm",
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

/// Wrap a fallible bootstrap stage: on `Err`, emit
/// `jdk.vendor_repo.bootstrap_failed { stage }` before propagating.
#[cfg(target_os = "linux")]
fn stage<T>(
    vendor: &str,
    pm: &str,
    stage_name: &'static str,
    r: Result<T, InstallError>,
) -> Result<T, InstallError> {
    r.map_err(|e| {
        emit_bootstrap_failed(vendor, pm, stage_name, &format!("{e}"));
        e
    })
}

/// Read the fingerprint of the key at `key_path` via
/// `gpg --show-keys --with-fingerprint --with-colons` and compare
/// against `expected` (case-insensitive, whitespace-stripped). On
/// mismatch, remove the just-written key file and return
/// `InstallError::Prereq`. When `expected == "UNVERIFIED"`, log a
/// warning and skip the check.
#[cfg(target_os = "linux")]
fn verify_fingerprint(repo: &VendorRepo, pm: &str) -> Result<(), InstallError> {
    use crate::tools::common::{default_use_sudo, run_maybe_sudo};

    if repo.expected_fingerprint == "UNVERIFIED" {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "jdk.vendor_repo.fingerprint_check_skipped",
                vendor = repo.vendor,
                pm = pm,
            );
        }
        return Ok(());
    }

    let out = run(
        "gpg",
        &[
            "--show-keys",
            "--with-fingerprint",
            "--with-colons",
            repo.key_path,
        ],
    )?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let actual = stdout
        .lines()
        .find_map(|line| {
            let mut it = line.split(':');
            if it.next() == Some("fpr") {
                // colons format: fpr:::::::::<fingerprint>:
                it.nth(8).map(str::to_string)
            } else {
                None
            }
        })
        .unwrap_or_default();

    let actual_norm = actual.trim().to_ascii_uppercase();
    let expected_norm = repo.expected_fingerprint.trim().to_ascii_uppercase();

    if actual_norm != expected_norm {
        emit_bootstrap_failed(
            repo.vendor,
            pm,
            "fingerprint_mismatch",
            &format!("expected {expected_norm}, got {actual_norm}"),
        );
        // Best-effort scrub of the poisoned key so a follow-up run
        // doesn't inherit it.
        let use_sudo = default_use_sudo().unwrap_or(true);
        let _ = run_maybe_sudo(use_sudo, "rm", &["-f", repo.key_path]);
        return Err(InstallError::Prereq(std::borrow::Cow::Owned(format!(
            "GPG fingerprint mismatch for {}: expected {}, got {} - refusing to install repo",
            repo.vendor, expected_norm, actual_norm
        ))));
    }
    Ok(())
}

/// Fetch the vendor GPG key over HTTPS + dearmor it into `key_path`
/// under sudo. Fails with `Prereq` if curl / gpg / lsb_release are
/// missing — never auto-installs them silently.
#[cfg(target_os = "linux")]
fn bootstrap_apt_repo(repo: &VendorRepo) -> Result<(), InstallError> {
    use crate::tools::common::{default_use_sudo, run, run_maybe_sudo};

    let started = std::time::Instant::now();
    emit_bootstrap_started(repo.vendor, "apt");

    if repo_recently_written(repo.list_path) {
        emit_bootstrap_skipped(repo.vendor, "apt", "recent_mtime");
        return Ok(());
    }

    stage(
        repo.vendor,
        "apt",
        "require_curl",
        require("curl", "curl is required to fetch the vendor GPG key"),
    )?;
    stage(
        repo.vendor,
        "apt",
        "require_gpg",
        require("gpg", "gpg is required to dearmor the vendor GPG key"),
    )?;
    stage(
        repo.vendor,
        "apt",
        "require_lsb_release",
        require(
            "lsb_release",
            "lsb_release not found (install lsb-release) - required to detect the Debian/Ubuntu codename",
        ),
    )?;

    let use_sudo = default_use_sudo().unwrap_or(true);

    // Resolve codename before doing any writes.
    let codename_out = stage(
        repo.vendor,
        "apt",
        "resolve_codename",
        run("lsb_release", &["-cs"]),
    )?;
    let codename_raw = String::from_utf8_lossy(&codename_out.stdout);
    let codename = codename_raw.trim();
    if !is_valid_debian_codename(codename) {
        emit_bootstrap_failed(
            repo.vendor,
            "apt",
            "codename_refused",
            "lsb_release codename failed validation",
        );
        return Err(InstallError::Prereq(
            "lsb_release returned a codename with unexpected characters; refusing to bootstrap vendor repo".into(),
        ));
    }

    // Ensure key parent dir exists (e.g. /etc/apt/keyrings on newer
    // Debian derivatives).
    if let Some(parent) = std::path::Path::new(repo.key_path).parent() {
        let parent_str = parent.to_string_lossy();
        stage(
            repo.vendor,
            "apt",
            "mkdir_key_parent",
            run_maybe_sudo(use_sudo, "mkdir", &["-p", &parent_str]),
        )?;
    }

    // Fetch the key to a tmp file, then dearmor via a shell pipeline
    // that itself runs under sudo so the resulting file lands with
    // root ownership at `key_path`. The pipeline body uses ONLY
    // `&'static str` inputs from `VendorRepo` and the validated
    // codename — no user string reaches the shell. curl is hardened
    // to HTTPS-only, TLS 1.2+, and zero redirects.
    let dearmor_cmd = format!(
        "set -euo pipefail; curl -fsSL --proto '=https' --tlsv1.2 --max-redirs 0 {} | gpg --dearmor --yes -o {}",
        shell_single_quote(repo.key_url),
        shell_single_quote(repo.key_path)
    );
    stage(
        repo.vendor,
        "apt",
        "fetch_key",
        run_maybe_sudo(use_sudo, "sh", &["-c", &dearmor_cmd]),
    )?;

    stage(
        repo.vendor,
        "apt",
        "fingerprint_mismatch",
        verify_fingerprint(repo, "apt"),
    )?;

    // Write the sources.list file via `sudo tee`. Body is a static
    // const with one `%CODENAME%` substitution; the substitution has
    // been validated above.
    let body = repo.body.replace("%CODENAME%", codename);
    let write_cmd = format!(
        "printf '%s' {} | tee {} > /dev/null",
        shell_single_quote(&body),
        shell_single_quote(repo.list_path)
    );
    stage(
        repo.vendor,
        "apt",
        "write_sources_list",
        run_maybe_sudo(use_sudo, "sh", &["-c", &write_cmd]),
    )?;

    // Refresh apt indexes so the newly-added repo is visible to the
    // subsequent `PkgOps::install` call.
    let apt = if has("apt") { "apt" } else { "apt-get" };
    stage(
        repo.vendor,
        "apt",
        "apt_update",
        run_maybe_sudo(use_sudo, apt, &["update"]),
    )?;
    mark_pm_updated(&APT_UPDATED);

    emit_bootstrap_completed(repo.vendor, "apt", started.elapsed().as_millis() as u64);
    Ok(())
}

#[cfg(target_os = "linux")]
fn bootstrap_dnf_repo(repo: &VendorRepo) -> Result<(), InstallError> {
    use crate::tools::common::{default_use_sudo, run_maybe_sudo};

    let started = std::time::Instant::now();
    emit_bootstrap_started(repo.vendor, "dnf");

    if repo_recently_written(repo.list_path) {
        emit_bootstrap_skipped(repo.vendor, "dnf", "recent_mtime");
        return Ok(());
    }

    stage(
        repo.vendor,
        "dnf",
        "require_curl",
        require("curl", "curl is required to fetch the vendor GPG key"),
    )?;
    // gpg is needed to inspect the fingerprint even though dnf
    // consumes the ASCII-armored key directly.
    stage(
        repo.vendor,
        "dnf",
        "require_gpg",
        require(
            "gpg",
            "gpg is required to verify the vendor GPG key fingerprint",
        ),
    )?;

    let use_sudo = default_use_sudo().unwrap_or(true);

    if let Some(parent) = std::path::Path::new(repo.key_path).parent() {
        let parent_str = parent.to_string_lossy();
        stage(
            repo.vendor,
            "dnf",
            "mkdir_key_parent",
            run_maybe_sudo(use_sudo, "mkdir", &["-p", &parent_str]),
        )?;
    }

    // dnf accepts ASCII-armored keys directly (gpgkey=file://…), so
    // no dearmor step is required — just fetch to key_path under sudo.
    // curl is hardened to HTTPS-only, TLS 1.2+, and zero redirects.
    let fetch_cmd = format!(
        "set -euo pipefail; curl -fsSL --proto '=https' --tlsv1.2 --max-redirs 0 {} -o {}",
        shell_single_quote(repo.key_url),
        shell_single_quote(repo.key_path)
    );
    stage(
        repo.vendor,
        "dnf",
        "fetch_key",
        run_maybe_sudo(use_sudo, "sh", &["-c", &fetch_cmd]),
    )?;

    stage(
        repo.vendor,
        "dnf",
        "fingerprint_mismatch",
        verify_fingerprint(repo, "dnf"),
    )?;

    let write_cmd = format!(
        "printf '%s' {} | tee {} > /dev/null",
        shell_single_quote(repo.body),
        shell_single_quote(repo.list_path)
    );
    stage(
        repo.vendor,
        "dnf",
        "write_repo_file",
        run_maybe_sudo(use_sudo, "sh", &["-c", &write_cmd]),
    )?;

    // dnf makecache so the newly-added repo is visible to the
    // subsequent install call.
    let dnf = if has("dnf") { "dnf" } else { "yum" };
    stage(
        repo.vendor,
        "dnf",
        "makecache",
        run_maybe_sudo(use_sudo, dnf, &["makecache"]),
    )?;
    mark_pm_updated(&DNF_UPDATED);

    emit_bootstrap_completed(repo.vendor, "dnf", started.elapsed().as_millis() as u64);
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
        (Openjdk, VersionSelector::Latest) => Winget(format!("Microsoft.OpenJDK.{DEFAULT_MAJOR}")),
        (Openjdk, VersionSelector::Major(n)) => Winget(format!("Microsoft.OpenJDK.{n}")),
        (Temurin, VersionSelector::Latest) => {
            Winget(format!("EclipseAdoptium.Temurin.{DEFAULT_MAJOR}.JDK"))
        }
        (Temurin, VersionSelector::Major(n)) => Winget(format!("EclipseAdoptium.Temurin.{n}.JDK")),
        (Zulu, VersionSelector::Latest) => Winget(format!("Azul.Zulu.{DEFAULT_MAJOR}.JDK")),
        (Zulu, VersionSelector::Major(n)) => Winget(format!("Azul.Zulu.{n}.JDK")),
        (Corretto, VersionSelector::Latest) => {
            Winget(format!("Amazon.Corretto.{DEFAULT_MAJOR}.JDK"))
        }
        (Corretto, VersionSelector::Major(n)) => Winget(format!("Amazon.Corretto.{n}.JDK")),
        (Liberica, VersionSelector::Latest) => {
            Winget(format!("BellSoft.LibericaJDK.{DEFAULT_MAJOR}.Full"))
        }
        (Liberica, VersionSelector::Major(n)) => Winget(format!("BellSoft.LibericaJDK.{n}.Full")),
        (MicrosoftOpenjdk, VersionSelector::Latest) => {
            Winget(format!("Microsoft.OpenJDK.{DEFAULT_MAJOR}"))
        }
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
    /// exact-version OpenJDK route.
    FallbackToDefault {
        requested: String,
        reason_kind: &'static str,
        reason: String,
        /// Exact OpenJDK route for the originally requested version.
        /// Keeping the resolved route in the decision prevents fallback
        /// from silently dropping a major-version pin.
        route: InstallRoute,
    },
    /// Requested distro unsupported AND `fallback = false`; hard-error.
    Refuse {
        requested: String,
        reason_kind: &'static str,
        reason: String,
    },
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
                fallback_decision(requested, "unknown_distribution", reason, ver)
            } else {
                Decision::Refuse {
                    requested: requested.to_string(),
                    reason_kind: "unknown_distribution",
                    reason,
                }
            }
        }
        Some(distro) => {
            let route = resolve_route(distro, ver);
            match route {
                InstallRoute::Unsupported {
                    reason_kind,
                    reason,
                } => {
                    if fallback {
                        fallback_decision(requested, reason_kind, reason, ver)
                    } else {
                        Decision::Refuse {
                            requested: requested.to_string(),
                            reason_kind,
                            reason,
                        }
                    }
                }
                other => Decision::RunRoute(other),
            }
        }
    }
}

/// Resolve fallback to OpenJDK without losing the requested version.
/// If this platform cannot provision that exact OpenJDK major, refuse
/// instead of installing an unversioned/default JDK and claiming success.
fn fallback_decision(
    requested: &str,
    reason_kind: &'static str,
    reason: String,
    ver: VersionSelector,
) -> Decision {
    match resolve_route(JdkDistribution::Openjdk, ver) {
        InstallRoute::Unsupported {
            reason: fallback_reason,
            ..
        } => Decision::Refuse {
            requested: requested.to_string(),
            reason_kind: "exact_fallback_unavailable",
            reason: format!("{reason}; exact OpenJDK fallback is unavailable: {fallback_reason}"),
        },
        route => Decision::FallbackToDefault {
            requested: requested.to_string(),
            reason_kind,
            reason,
            route,
        },
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
        reason_kind: "unsupported_os",
        reason: format!(
            "distribution '{}' has no route on this OS",
            distro.as_slug()
        ),
    }
}

fn install_java(min_hint: &str, ctx: &InstallContext) -> Result<(), InstallError> {
    let requested = ctx.distribution.as_deref().unwrap_or("openjdk");

    match decide(requested, min_hint, ctx.fallback) {
        Decision::RunRoute(route) => {
            // Selected event fires ONLY when a route actually runs —
            // makes the counter a true "selected + executed" count.
            let selected_slug = JdkDistribution::parse(requested)
                .map(|d| d.as_slug())
                .unwrap_or("openjdk");
            emit_distribution_selected(selected_slug, min_hint);
            execute_route(route)
        }
        Decision::FallbackToDefault {
            requested,
            reason_kind,
            reason,
            route,
        } => {
            eprintln!("warn: {reason}. Falling back to openjdk.");
            emit_distribution_fallback(&requested, true, reason_kind, &reason);
            execute_route(route)
        }
        Decision::Refuse {
            requested,
            reason_kind,
            reason,
        } => {
            emit_distribution_fallback(&requested, false, reason_kind, &reason);
            Err(InstallError::Prereq(std::borrow::Cow::Owned(format!(
                "Java distribution '{requested}' cannot be provisioned with the requested version; no safe fallback: {reason}"
            ))))
        }
    }
}

/// Render the exact install command selected for Java without executing it.
/// Used by `setup --dry-run` so the preview and real installer share the
/// same version-aware decision instead of showing the static `openjdk` slot.
pub fn preview_install(min_hint: &str, ctx: &InstallContext) -> String {
    let requested = ctx.distribution.as_deref().unwrap_or("openjdk");
    match decide(requested, min_hint, ctx.fallback) {
        Decision::RunRoute(route) => format!("`{}`", route_command(&route)),
        Decision::FallbackToDefault { reason, route, .. } => {
            // `reason` embeds the raw user-supplied `distribution` field
            // (via `format!("unknown distribution '{requested}'")` in
            // decide()). A hostile local jarvy.toml with ANSI escapes in
            // `distribution` would clear the operator's terminal on
            // `setup --dry-run`. Launder through the shared sanitizer.
            let safe = crate::tools::unsupported::sanitize_for_display(&reason);
            format!("`{}` (OpenJDK fallback: {})", route_command(&route), safe)
        }
        Decision::Refuse { reason, .. } => {
            let safe = crate::tools::unsupported::sanitize_for_display(&reason);
            format!("REFUSED ({safe})")
        }
    }
}

fn route_command(route: &InstallRoute) -> String {
    match route {
        InstallRoute::BrewFormula(pkg) => format!("brew install {pkg}"),
        InstallRoute::BrewCask(pkg) => format!("brew install --cask {pkg}"),
        InstallRoute::AptPkg { pkg, .. } => format!("apt-get install -y {pkg}"),
        InstallRoute::DnfPkg { pkg, .. } => format!("dnf install -y {pkg}"),
        InstallRoute::Winget(id) => format!("winget install -e --id {id}"),
        InstallRoute::Choco(id) => format!("choco install -y {id}"),
        InstallRoute::Unsupported { reason, .. } => format!("unsupported: {reason}"),
    }
}

/// Emit `tool.distribution_selected` via the tracing subscriber.
/// Library code cannot reach `crate::telemetry` (bin-only module), so
/// events are emitted directly and the OTLP layer picks them up.
/// Gated on the crate-wide consent gate so `telemetry.enabled = false`
/// silences these events even when an OTLP endpoint is set for
/// unrelated reasons. Fields are bounded-cardinality only:
/// `distribution` is a slug from a closed 7-variant enum,
/// `version_kind` is one of `"latest"|"major"`, `os` is
/// `std::env::consts::OS` (~5 values). The raw user version string is
/// deliberately NOT emitted — it's cardinality-unbounded user input.
fn emit_distribution_selected(distribution: &str, version_input: &str) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    let version_kind = match normalize_version(version_input) {
        VersionSelector::Latest => "latest",
        VersionSelector::Major(_) => "major",
    };
    tracing::info!(
        event = "tool.distribution_selected",
        tool = "java",
        distribution = distribution,
        version_kind = version_kind,
        os = std::env::consts::OS,
    );
}

/// Emit `tool.distribution_fallback`. `reason` is human-readable text
/// (kept low-cardinality-ish because callsite reasons are constants /
/// small format templates); `reason_kind` is the bounded label used
/// for aggregation. `parsed_kind` collapses `requested` to
/// `"known"|"unknown"` so the raw user string never becomes a metric
/// attribute.
fn emit_distribution_fallback(
    requested: &str,
    fell_back: bool,
    reason_kind: &'static str,
    reason: &str,
) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    let parsed_kind = if JdkDistribution::parse(requested).is_some() {
        "known"
    } else {
        "unknown"
    };
    tracing::warn!(
        event = "tool.distribution_fallback",
        tool = "java",
        parsed_kind = parsed_kind,
        fell_back = fell_back,
        reason_kind = reason_kind,
        os = std::env::consts::OS,
        hint = reason,
    );
}

#[allow(dead_code)]
fn emit_bootstrap_started(vendor: &str, pm: &str) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(
        event = "jdk.vendor_repo.bootstrap_started",
        vendor = vendor,
        pm = pm
    );
}

#[allow(dead_code)]
fn emit_bootstrap_completed(vendor: &str, pm: &str, duration_ms: u64) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(
        event = "jdk.vendor_repo.bootstrapped",
        vendor = vendor,
        pm = pm,
        duration_ms = duration_ms,
    );
}

#[allow(dead_code)]
fn emit_bootstrap_skipped(vendor: &str, pm: &str, reason: &'static str) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::debug!(
        event = "jdk.vendor_repo.bootstrap_skipped",
        vendor = vendor,
        pm = pm,
        reason = reason,
    );
}

#[allow(dead_code)]
fn emit_bootstrap_failed(vendor: &str, pm: &str, stage: &'static str, error: &str) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::warn!(
        event = "jdk.vendor_repo.bootstrap_failed",
        vendor = vendor,
        pm = pm,
        stage = stage,
        error = error,
    );
}

fn execute_route(route: InstallRoute) -> Result<(), InstallError> {
    match route {
        InstallRoute::BrewFormula(pkg) => brew_install(&["install", &pkg]),
        InstallRoute::BrewCask(pkg) => brew_install(&["install", "--cask", &pkg]),
        InstallRoute::AptPkg { pkg, repo } => {
            #[cfg(target_os = "linux")]
            {
                if let Some(r) = repo {
                    bootstrap_apt_repo(r)?;
                }
                apt_install(&pkg)
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (pkg, repo);
                Err(InstallError::Unsupported)
            }
        }
        InstallRoute::DnfPkg { pkg, repo } => {
            #[cfg(target_os = "linux")]
            {
                if let Some(r) = repo {
                    bootstrap_dnf_repo(r)?;
                }
                dnf_install(&pkg)
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (pkg, repo);
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
            "Homebrew not found. Install https://brew.sh and re-run.".into(),
        ));
    }
    run("brew", args)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn apt_install(pkg: &str) -> Result<(), InstallError> {
    use crate::tools::common::{PackageManager, PkgOps, default_use_sudo};
    if !pm_already_updated(&APT_UPDATED) {
        let _ = PkgOps::update(PackageManager::Apt, default_use_sudo());
        mark_pm_updated(&APT_UPDATED);
    }
    PkgOps::install(PackageManager::Apt, pkg, default_use_sudo())
}

#[cfg(target_os = "linux")]
fn dnf_install(pkg: &str) -> Result<(), InstallError> {
    use crate::tools::common::{PackageManager, PkgOps, default_use_sudo};
    if !pm_already_updated(&DNF_UPDATED) {
        let _ = PkgOps::update(PackageManager::Dnf, default_use_sudo());
        mark_pm_updated(&DNF_UPDATED);
    }
    PkgOps::install(PackageManager::Dnf, pkg, default_use_sudo())
}

fn winget_install(id: &str) -> Result<(), InstallError> {
    if !has("winget") {
        return Err(InstallError::Prereq(
            "winget not found. Install Windows Package Manager, then re-run.".into(),
        ));
    }
    run("winget", &["install", "-e", "--id", id])?;
    Ok(())
}

fn choco_install(id: &str) -> Result<(), InstallError> {
    crate::tools::chocolatey::ensure_installed()?;
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
            InstallRoute::AptPkg {
                pkg: "temurin-17-jdk".into(),
                repo: Some(&TEMURIN_APT),
            }
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
            InstallRoute::DnfPkg {
                pkg: "temurin-21-jdk".into(),
                repo: Some(&TEMURIN_DNF),
            }
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
            InstallRoute::AptPkg {
                pkg: "zulu17-jdk".into(),
                repo: Some(&ZULU_APT),
            }
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
            InstallRoute::DnfPkg {
                pkg: "zulu17-jdk".into(),
                repo: Some(&ZULU_DNF),
            }
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
            InstallRoute::AptPkg {
                pkg: "java-17-amazon-corretto-jdk".into(),
                repo: Some(&CORRETTO_APT),
            }
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
            InstallRoute::DnfPkg {
                pkg: "java-17-amazon-corretto-devel".into(),
                repo: Some(&CORRETTO_DNF),
            }
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
            InstallRoute::AptPkg {
                pkg: "bellsoft-java17-full".into(),
                repo: Some(&LIBERICA_APT),
            }
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
            InstallRoute::DnfPkg {
                pkg: "bellsoft-java17-full".into(),
                repo: Some(&LIBERICA_DNF),
            }
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
            InstallRoute::Unsupported {
                reason_kind,
                reason,
            } => {
                assert_eq!(reason_kind, "msopenjdk_apt_manual");
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
            InstallRoute::DnfPkg {
                pkg: "msopenjdk-21".into(),
                repo: Some(&MSOPENJDK_DNF),
            }
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
            InstallRoute::AptPkg {
                pkg: format!("temurin-{DEFAULT_MAJOR}-jdk"),
                repo: Some(&TEMURIN_APT),
            }
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
                InstallRoute::Unsupported { reason_kind, .. } => {
                    assert!(
                        !reason_kind.is_empty(),
                        "Unsupported must carry a bounded reason_kind"
                    );
                }
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

    #[cfg(target_os = "windows")]
    #[test]
    fn resolve_windows_uses_default_major() {
        // Every Latest arm on Windows should interpolate DEFAULT_MAJOR
        // rather than a hardcoded literal.
        let expected = format!("Microsoft.OpenJDK.{DEFAULT_MAJOR}");
        assert_eq!(
            resolve_route(JdkDistribution::Openjdk, VersionSelector::Latest),
            InstallRoute::Winget(expected.clone())
        );
        assert_eq!(
            resolve_route(JdkDistribution::MicrosoftOpenjdk, VersionSelector::Latest),
            InstallRoute::Winget(expected)
        );
        assert_eq!(
            resolve_route(JdkDistribution::Temurin, VersionSelector::Latest),
            InstallRoute::Winget(format!("EclipseAdoptium.Temurin.{DEFAULT_MAJOR}.JDK"))
        );
    }

    #[test]
    fn decide_unknown_distribution_falls_back_when_allowed() {
        match decide("bogus-jdk", "17", true) {
            Decision::FallbackToDefault {
                requested,
                reason_kind,
                reason,
                route,
            } => {
                assert_eq!(requested, "bogus-jdk");
                assert_eq!(reason_kind, "unknown_distribution");
                assert!(reason.contains("unknown distribution"));
                assert!(
                    route_command(&route).contains("17"),
                    "fallback must preserve the requested major: {route:?}"
                );
            }
            other => panic!("expected FallbackToDefault, got {other:?}"),
        }
    }

    #[test]
    fn java_17_preview_names_exact_versioned_package() {
        let preview = preview_install("17", &InstallContext::none());

        assert!(
            preview.contains("17"),
            "preview must expose an exact JDK 17 route: {preview}"
        );
        #[cfg(target_os = "macos")]
        assert_eq!(preview, "`brew install openjdk@17`");
        #[cfg(target_os = "windows")]
        assert_eq!(preview, "`winget install -e --id Microsoft.OpenJDK.17`");
    }

    #[test]
    fn decide_unknown_distribution_refuses_when_fallback_false() {
        match decide("bogus-jdk", "17", false) {
            Decision::Refuse {
                requested,
                reason_kind,
                reason,
            } => {
                assert_eq!(requested, "bogus-jdk");
                assert_eq!(reason_kind, "unknown_distribution");
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
                    InstallRoute::BrewFormula(s) | InstallRoute::BrewCask(s) => {
                        !s.contains(';') && !s.contains('|') && !s.contains('`')
                    }
                    InstallRoute::AptPkg { pkg, .. } | InstallRoute::DnfPkg { pkg, .. } => {
                        !pkg.contains(';') && !pkg.contains('|') && !pkg.contains('`')
                    }
                    InstallRoute::Winget(s) | InstallRoute::Choco(s) => {
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

    #[test]
    fn install_route_unsupported_carries_reason_kind() {
        // Every Unsupported constructed by the router must set a
        // non-empty bounded reason_kind. Exercise every platform via
        // the pure per-platform routers; on macOS every arm is a
        // supported route, so the assertion is trivially true (no
        // Unsupported reachable).
        #[cfg(target_os = "linux")]
        {
            let cases: &[(Option<PackageManager>, JdkDistribution)] = &[
                (Some(PackageManager::Pacman), JdkDistribution::Temurin),
                (Some(PackageManager::Apk), JdkDistribution::Zulu),
                (None, JdkDistribution::Openjdk),
                (Some(PackageManager::Apt), JdkDistribution::MicrosoftOpenjdk),
            ];
            let allowed = ["unsupported_pm", "unsupported_os", "msopenjdk_apt_manual"];
            for (pm, distro) in cases {
                if let InstallRoute::Unsupported { reason_kind, .. } =
                    resolve_linux_inner(*pm, *distro, VersionSelector::Major(17))
                {
                    assert!(
                        allowed.contains(&reason_kind),
                        "unexpected reason_kind={reason_kind:?} for ({pm:?}, {distro:?})"
                    );
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn vendor_repo_expected_fingerprint_shape() {
        for repo in [
            &TEMURIN_APT,
            &TEMURIN_DNF,
            &ZULU_APT,
            &ZULU_DNF,
            &CORRETTO_APT,
            &CORRETTO_DNF,
            &LIBERICA_APT,
            &LIBERICA_DNF,
            &MSOPENJDK_DNF,
        ] {
            let fpr = repo.expected_fingerprint;
            let ok = fpr == "UNVERIFIED"
                || (fpr.len() == 40 && fpr.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(
                ok,
                "vendor {} has invalid expected_fingerprint: {:?}",
                repo.vendor, fpr
            );
        }
    }

    #[test]
    fn preview_refuses_when_openjdk_route_unsupported_and_fallback_false() {
        // `preview_install` must render the Refuse arm literally, not
        // silently produce an empty backtick pair — the dry-run preview
        // is the operator's last chance to see what setup will do.
        let ctx = InstallContext {
            distribution: Some("bogus-jdk".to_string()),
            fallback: false,
        };
        let preview = preview_install("17", &ctx);
        assert!(
            preview.starts_with("REFUSED"),
            "preview must render Refuse explicitly: {preview}"
        );
        assert!(
            preview.contains("bogus-jdk") || preview.contains("unknown"),
            "preview must name the refusal reason: {preview}"
        );
    }

    #[test]
    fn preview_fallback_arm_names_openjdk_and_target_route() {
        // When fallback = true, preview must show BOTH the fallback
        // marker and the exact OpenJDK route that will run — otherwise
        // dry-run and apply disagree on the resolved package.
        let ctx = InstallContext {
            distribution: Some("bogus-jdk".to_string()),
            fallback: true,
        };
        let preview = preview_install("17", &ctx);
        assert!(
            preview.contains("OpenJDK fallback"),
            "preview must announce fallback: {preview}"
        );
        assert!(
            preview.contains("17"),
            "preview must preserve the requested major version: {preview}"
        );
    }

    #[test]
    fn preview_sanitizes_ansi_escapes_in_distribution_reason() {
        // A hostile local jarvy.toml with ANSI in the distribution field
        // must not clear the operator's terminal when dry-run prints
        // the preview. `sanitize_for_display` replaces control bytes
        // with `?` — verify no ESC (0x1b) survives to the output.
        let ctx = InstallContext {
            distribution: Some("\x1b[2Jbogus".to_string()),
            fallback: false,
        };
        let preview = preview_install("17", &ctx);
        assert!(
            !preview.contains('\x1b'),
            "preview must strip ESC bytes from user-supplied distribution: {preview:?}"
        );
    }

    #[test]
    fn install_java_refuse_error_names_requested_and_fallback_false() {
        // Unknown distribution is deterministic across all OSes and
        // never touches a real installer. Pins the CLI-facing error
        // string so a regression cannot silently reword the message.
        let ctx = InstallContext {
            distribution: Some("nonexistent-jdk".to_string()),
            fallback: false,
        };
        let err = install_java("17", &ctx)
            .expect_err("unknown distribution with fallback=false must refuse");
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent-jdk") || msg.contains("nonexistent"),
            "error must name the requested distribution: got {msg}"
        );
        assert!(
            msg.contains("fallback") || msg.to_lowercase().contains("supported"),
            "error must explain the fallback refusal reason: got {msg}"
        );
    }
}
