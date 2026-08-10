//! Android SDK Command-Line Tools - SDK and virtual-device management
//!
//! Platform Tools only provide device-facing commands such as `adb` and
//! `fastboot`. This package adds `sdkmanager`, `avdmanager`, Android lint,
//! and the other commands needed to install SDK platforms, build-tools,
//! system images, and emulators.
//!
//! Google publishes versioned archive filenames and SHA-256 checksums on the
//! Android Studio download page. Keep the revision, URLs, and hashes below in
//! lockstep when refreshing the pin. The `_latest` suffix is part of Google's
//! filename, but the numeric revision and checksum make this installation
//! immutable and integrity-gated.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};

const REVISION: &str = "15859902";
const BASE_URL: &str = "https://dl.google.com/android/repository";

const LINUX_X64_SHA256: &str = "4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583";
const MAC_X64_SHA256: &str = "c5a6378ab5cf7e0d5701921405115befff13e9ff7417fb588389338f8bd050f3";
const MAC_ARM64_SHA256: &str = "835b62a26162b229b441d1f6d4680383815a270809eb33522c0d480fa5002c4e";
const WINDOWS_X64_SHA256: &str = "90ae805d20434428bffcb699c290860f19bb5f66a67e6b330067e3de801fb04a";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Artifact {
    filename: &'static str,
    sha256: &'static str,
    default_sdk_root: &'static str,
}

fn artifact_for(os: &str, arch: &str) -> Option<Artifact> {
    match (os, arch) {
        ("linux", "x86_64") => Some(Artifact {
            filename: "commandlinetools-linux-15859902_latest.zip",
            sha256: LINUX_X64_SHA256,
            default_sdk_root: "$HOME/Android/Sdk",
        }),
        ("macos", "x86_64") => Some(Artifact {
            filename: "commandlinetools-mac_x86_64-15859902_latest.zip",
            sha256: MAC_X64_SHA256,
            default_sdk_root: "$HOME/Library/Android/sdk",
        }),
        ("macos", "aarch64") => Some(Artifact {
            filename: "commandlinetools-mac_arm64-15859902_latest.zip",
            sha256: MAC_ARM64_SHA256,
            default_sdk_root: "$HOME/Library/Android/sdk",
        }),
        ("windows", "x86_64") => Some(Artifact {
            filename: "commandlinetools-win-15859902_latest.zip",
            sha256: WINDOWS_X64_SHA256,
            default_sdk_root: r"(Join-Path $env:LOCALAPPDATA 'Android\Sdk')",
        }),
        _ => None,
    }
}

define_tool!(ANDROID_COMMAND_LINE_TOOLS, {
    command: "sdkmanager",
    custom_install: install_android_command_line_tools,
    depends_on: &["java"],
    category: "mobile",
});

fn install_android_command_line_tools(_min_hint: &str) -> Result<(), InstallError> {
    if has("sdkmanager") {
        return Ok(());
    }

    let artifact = artifact_for(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or(InstallError::Unsupported)?;
    let url = format!("{BASE_URL}/{}", artifact.filename);

    #[cfg(unix)]
    {
        if !has("curl") {
            return Err(InstallError::Prereq(
                "curl is required to install Android command-line tools",
            ));
        }
        if !has("unzip") {
            return Err(InstallError::Prereq(
                "unzip is required to install Android command-line tools",
            ));
        }
        let script = unix_install_script(&url, artifact);
        run("sh", &["-c", &script])?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let script = windows_install_script(&url, artifact);
        run(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", &script],
        )?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[allow(dead_code)] // compiled on Windows for cross-platform string tests
fn unix_install_script(url: &str, artifact: Artifact) -> String {
    format!(
        r#"set -eu
SDK_ROOT="${{ANDROID_HOME:-{default_sdk_root}}}"
TOOLS_ROOT="$SDK_ROOT/cmdline-tools"
BIN_DIR="${{XDG_BIN_HOME:-$HOME/.local/bin}}"
ARCHIVE=$(mktemp)
STAGE=$(mktemp -d)
trap 'rm -f "$ARCHIVE"; rm -rf "$STAGE"' EXIT
curl -fsSL '{url}' -o "$ARCHIVE"
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL=$(sha256sum "$ARCHIVE" | cut -d' ' -f1)
else
  ACTUAL=$(shasum -a 256 "$ARCHIVE" | cut -d' ' -f1)
fi
EXPECTED='{sha256}'
if [ "$ACTUAL" != "$EXPECTED" ]; then
  printf 'jarvy: refusing to install Android command-line tools; sha256 mismatch (got %s, want %s)\n' \
      "$ACTUAL" "$EXPECTED" >&2
  exit 1
fi
unzip -q "$ARCHIVE" -d "$STAGE"
mkdir -p "$TOOLS_ROOT" "$BIN_DIR"
rm -rf "$TOOLS_ROOT/latest"
mv "$STAGE/cmdline-tools" "$TOOLS_ROOT/latest"
for TOOL in sdkmanager avdmanager apkanalyzer lint retrace; do
  ln -sfn "$TOOLS_ROOT/latest/bin/$TOOL" "$BIN_DIR/$TOOL"
done
MARKER='# Jarvy Android SDK'
for RC in "$HOME/.bashrc" "$HOME/.zshrc"; do
  if [ -f "$RC" ] && ! grep -Fq "$MARKER" "$RC"; then
    printf '\n%s\nexport ANDROID_HOME="%s"\nexport PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"\n' \
        "$MARKER" "$SDK_ROOT" >> "$RC"
  fi
done
printf 'jarvy: installed Android command-line tools revision {revision} under %s\n' "$SDK_ROOT"
printf 'jarvy: restart your shell, then use sdkmanager to install SDK platforms and build-tools\n'
"#,
        default_sdk_root = artifact.default_sdk_root,
        url = url,
        sha256 = artifact.sha256,
        revision = REVISION,
    )
}

#[allow(dead_code)] // compiled on Unix for cross-platform string tests
fn windows_install_script(url: &str, artifact: Artifact) -> String {
    format!(
        r#"$ErrorActionPreference = 'Stop'
$SdkRoot = if ($env:ANDROID_HOME) {{ $env:ANDROID_HOME }} else {{ {default_sdk_root} }}
$Archive = Join-Path ([IO.Path]::GetTempPath()) (([IO.Path]::GetRandomFileName()) + '.zip')
$Stage = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
try {{
  Invoke-WebRequest -UseBasicParsing -Uri '{url}' -OutFile $Archive
  $Actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
  $Expected = '{sha256}'
  if ($Actual -ne $Expected) {{
    throw "jarvy: refusing to install Android command-line tools; sha256 mismatch (got $Actual, want $Expected)"
  }}
  Expand-Archive -LiteralPath $Archive -DestinationPath $Stage
  $ToolsRoot = Join-Path $SdkRoot 'cmdline-tools'
  $Latest = Join-Path $ToolsRoot 'latest'
  New-Item -ItemType Directory -Force -Path $ToolsRoot | Out-Null
  if (Test-Path $Latest) {{ Remove-Item -Recurse -Force $Latest }}
  Move-Item (Join-Path $Stage 'cmdline-tools') $Latest
  $ToolsBin = Join-Path $Latest 'bin'
  [Environment]::SetEnvironmentVariable('ANDROID_HOME', $SdkRoot, 'User')
  $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  $Entries = @($UserPath -split ';' | Where-Object {{ $_ }})
  if ($Entries -notcontains $ToolsBin) {{
    [Environment]::SetEnvironmentVariable('Path', (($Entries + $ToolsBin) -join ';'), 'User')
  }}
  Write-Host "jarvy: installed Android command-line tools revision {revision} under $SdkRoot"
  Write-Host 'jarvy: open a new terminal, then use sdkmanager to install SDK platforms and build-tools'
}} finally {{
  Remove-Item -Force -ErrorAction SilentlyContinue $Archive
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Stage
}}
"#,
        default_sdk_root = artifact.default_sdk_root,
        url = url,
        sha256 = artifact.sha256,
        revision = REVISION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_command_line_tools_registration_shape() {
        assert_eq!(ANDROID_COMMAND_LINE_TOOLS.command, "sdkmanager");
        assert_eq!(ANDROID_COMMAND_LINE_TOOLS.category, Some("mobile"));
        assert_eq!(ANDROID_COMMAND_LINE_TOOLS.depends_on, Some(&["java"][..]));
        assert!(ANDROID_COMMAND_LINE_TOOLS.custom_install.is_some());
    }

    #[test]
    fn official_artifact_matrix_is_pinned() {
        for (os, arch) in [
            ("linux", "x86_64"),
            ("macos", "x86_64"),
            ("macos", "aarch64"),
            ("windows", "x86_64"),
        ] {
            let artifact = artifact_for(os, arch).expect("supported artifact");
            assert!(artifact.filename.contains(REVISION));
            assert_eq!(artifact.sha256.len(), 64);
            assert!(
                artifact
                    .sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }
        assert!(artifact_for("linux", "aarch64").is_none());
        assert!(artifact_for("freebsd", "x86_64").is_none());
    }

    #[test]
    fn unix_installer_verifies_before_extracting() {
        let artifact = artifact_for("linux", "x86_64").unwrap();
        let script = unix_install_script("https://example.test/android.zip", artifact);
        let mismatch = script.find("sha256 mismatch").unwrap();
        let extract = script.find("unzip -q").unwrap();
        assert!(mismatch < extract);
        assert!(script.contains(artifact.sha256));
        assert!(script.contains("ANDROID_HOME"));
        assert!(script.contains("sdkmanager"));
    }

    #[test]
    fn windows_installer_verifies_before_extracting() {
        let artifact = artifact_for("windows", "x86_64").unwrap();
        let script = windows_install_script("https://example.test/android.zip", artifact);
        let mismatch = script.find("sha256 mismatch").unwrap();
        let extract = script.find("Expand-Archive").unwrap();
        assert!(mismatch < extract);
        assert!(script.contains(artifact.sha256));
        assert!(script.contains("ANDROID_HOME"));
        assert!(script.contains("SetEnvironmentVariable"));
    }
}
