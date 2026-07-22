use std::path::Path;
use std::process::{Command, Output};
use std::str;
use tracing::debug;

use crate::chatter;

pub(crate) fn handle_output(output: &Output) {
    if !output.status.success() {
        eprintln!("Error: {}", String::from_utf8_lossy(&output.stderr));
    }
}

fn get_cpu() -> String {
    let output = match Command::new("uname").arg("-m").output() {
        Ok(output) => output,
        Err(e) => {
            eprintln!("Failed to fetch CPU info: {e}");
            return String::new();
        }
    };

    if output.status.success() {
        let s = str::from_utf8(&output.stdout).unwrap_or_default();
        s.trim().to_string()
    } else {
        eprintln!("Error: {}", String::from_utf8_lossy(&output.stderr));
        String::new()
    }
}

// Read the current Finder `AppleShowAllFiles` setting. Returns true iff
// the key is already set to the truthy form (`YES` / `1` / `true`).
// `defaults read` exits non-zero when the key has never been written.
fn finder_shows_hidden() -> bool {
    let Ok(out) = Command::new("defaults")
        .args(["read", "com.apple.finder", "AppleShowAllFiles"])
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let v = String::from_utf8_lossy(&out.stdout)
        .trim()
        .to_ascii_lowercase();
    matches!(v.as_str(), "yes" | "1" | "true")
}

// Detect Rosetta 2. The dispatcher daemon `oahd` is registered via this
// libexec path; presence is the documented signal for "Rosetta 2 is
// installed" on macOS 11+. Cheaper than spawning `arch -x86_64 true`
// just to probe.
fn rosetta_installed() -> bool {
    Path::new("/Library/Apple/usr/libexec/oah/libRosettaRuntime").exists()
}

fn ensure_finder_shows_hidden() {
    if finder_shows_hidden() {
        debug!("Finder AppleShowAllFiles already YES; skipping");
        return;
    }
    chatter!("Configuring Finder to show hidden files");
    match Command::new("defaults")
        .args(["write", "com.apple.finder", "AppleShowAllFiles", "YES"])
        .output()
    {
        Ok(output) => handle_output(&output),
        Err(e) => eprintln!("Failed to execute defaults command: {e}"),
    }
}

fn ensure_rosetta_installed() {
    if get_cpu() == "arm64" && rosetta_installed() {
        debug!("Rosetta 2 already installed; skipping");
        return;
    }
    if get_cpu() == "arm64" {
        // Apple Silicon, no Rosetta yet — install non-interactively.
        // `--agree-to-license` skips the licence prompt that otherwise
        // blocks unattended setups.
        chatter!("Installing Rosetta 2 for x86_64 emulation");
        match Command::new("softwareupdate")
            .args(["--install-rosetta", "--agree-to-license"])
            .output()
        {
            Ok(output) => handle_output(&output),
            Err(e) => eprintln!("Failed to start Rosetta installation: {e}"),
        }
    }
    // On Intel hosts Rosetta is irrelevant — skip silently.
}

fn ensure_xcode_clt_installed() {
    let installed = Command::new("/usr/bin/xcode-select")
        .args(["-p"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if installed {
        // Silent on the happy path. CLT updates are intentionally
        // out of scope (see git log for 1a6564b — `softwareupdate -ia`
        // on Yes used to pull in a full macOS point release).
        debug!("Command Line Tools for Xcode already installed; skipping");
        return;
    }

    chatter!("Installing Command Line Tools for Xcode...");
    match Command::new("xcode-select").args(["--install"]).spawn() {
        Ok(mut child) => {
            if let Err(e) = child.wait() {
                eprintln!("Failed to wait on Xcode installation: {e}");
            }
        }
        Err(e) => eprintln!("Failed to start Xcode installation: {e}"),
    }
}

pub fn set_up_os(platform: &str) {
    match platform {
        "macos" => {
            ensure_finder_shows_hidden();
            ensure_rosetta_installed();
            ensure_xcode_clt_installed();
        }
        "linux" => {
            debug!("No OS-level configuration required on Linux");
        }
        "windows" => {
            chatter!("Set Windows system configurations");

            let output = match std::process::Command::new("powershell")
                .arg("/c")
                .arg("Set-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced' -Name 'Hidden' -Value 1")
                .output()
            {
                Ok(output) => output,
                Err(e) => {
                    eprintln!("Failed to execute powershell command: {e}");
                    return;
                }
            };

            if !output.status.success() {
                eprintln!("Error: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        _ => chatter!("Unsupported platform"),
    }
}
