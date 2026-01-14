//! Build script for Jarvy
//!
//! Generates a tool index JSON file at compile time by parsing
//! tool definitions from the source code.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Only rerun if tool source files change
    println!("cargo:rerun-if-changed=src/tools/mod.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("tools_index.json");

    // Parse tools from source and generate index
    let index = generate_build_time_index();

    fs::write(&dest_path, index).expect("Failed to write tools_index.json");

    // Also write to a predictable location for external tooling
    // This goes to target/tools_index.json
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let target_dir = Path::new(&manifest_dir).join("target");
        if target_dir.exists() {
            let target_index = target_dir.join("tools_index.json");
            let index = generate_build_time_index();
            let _ = fs::write(target_index, index);
        }
    }
}

/// Represents a tool's installation options for a platform.
#[derive(Debug, serde::Serialize)]
struct PlatformInstall {
    #[serde(skip_serializing_if = "Option::is_none")]
    brew: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cask: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dnf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    yum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zypper: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pacman: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    winget: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    choco: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uniform: Option<String>,
}

impl Default for PlatformInstall {
    fn default() -> Self {
        Self {
            brew: None,
            cask: None,
            apt: None,
            dnf: None,
            yum: None,
            zypper: None,
            pacman: None,
            apk: None,
            winget: None,
            choco: None,
            uniform: None,
        }
    }
}

/// A tool entry in the build-time index.
#[derive(Debug, serde::Serialize)]
struct BuildToolEntry {
    name: String,
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    macos: Option<PlatformInstall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    linux: Option<PlatformInstall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    windows: Option<PlatformInstall>,
    has_custom_installer: bool,
}

/// The complete build-time tool index.
#[derive(Debug, serde::Serialize)]
struct BuildToolIndex {
    version: String,
    generated_at: String,
    count: usize,
    tools: Vec<BuildToolEntry>,
}

/// Generate the tool index at build time by parsing source files.
fn generate_build_time_index() -> String {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let tools_dir = Path::new(&manifest_dir).join("src/tools");

    let mut tools: BTreeMap<String, BuildToolEntry> = BTreeMap::new();

    // Read the mod.rs to find all tool modules
    let mod_rs_path = tools_dir.join("mod.rs");
    if let Ok(mod_content) = fs::read_to_string(&mod_rs_path) {
        for line in mod_content.lines() {
            // Match lines like: pub mod git;
            if let Some(module_name) = parse_pub_mod(line) {
                // Skip non-tool modules
                if matches!(
                    module_name.as_str(),
                    "common" | "registry" | "spec" | "version"
                ) {
                    continue;
                }

                // Try to parse the tool definition file
                let tool_file = tools_dir
                    .join(&module_name)
                    .join(format!("{}.rs", module_name));
                if tool_file.exists() {
                    println!("cargo:rerun-if-changed={}", tool_file.display());
                    if let Ok(content) = fs::read_to_string(&tool_file) {
                        if let Some(entry) = parse_define_tool(&content, &module_name) {
                            tools.insert(entry.name.clone(), entry);
                        } else if is_custom_tool(&content) {
                            // Custom tool without define_tool! macro
                            tools.insert(
                                module_name.clone(),
                                BuildToolEntry {
                                    name: module_name.clone(),
                                    command: module_name.clone(),
                                    macos: None,
                                    linux: None,
                                    windows: None,
                                    has_custom_installer: true,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    let tools_vec: Vec<BuildToolEntry> = tools.into_values().collect();

    let index = BuildToolIndex {
        version: "1.0.0".to_string(),
        generated_at: chrono_lite_now(),
        count: tools_vec.len(),
        tools: tools_vec,
    };

    serde_json::to_string_pretty(&index).unwrap_or_else(|_| "{}".to_string())
}

/// Parse a `pub mod X;` line and return the module name.
fn parse_pub_mod(line: &str) -> Option<String> {
    let line = line.trim();
    if line.starts_with("pub mod ") && line.ends_with(';') {
        let name = line
            .strip_prefix("pub mod ")?
            .strip_suffix(';')?
            .trim()
            .to_string();
        Some(name)
    } else {
        None
    }
}

/// Parse a define_tool! macro invocation and extract tool information.
fn parse_define_tool(content: &str, module_name: &str) -> Option<BuildToolEntry> {
    // Find the define_tool! macro
    let start = content.find("define_tool!")?;
    let block_start = content[start..].find('{')?;
    let block_content = &content[start + block_start..];

    // Find matching brace - simple approach, count braces
    let mut depth = 0;
    let mut end_idx = 0;
    for (i, ch) in block_content.chars().enumerate() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = i;
                    break;
                }
            }
            _ => {}
        }
    }

    let block = &block_content[1..end_idx]; // Skip opening brace

    // Parse command
    let command = extract_string_value(block, "command:")?;

    // Parse platform options
    let macos = parse_macos_block(block);
    let linux = parse_linux_block(block);
    let windows = parse_windows_block(block);
    let has_custom = block.contains("custom_install:");

    // Tool name is the identifier in the macro (e.g., GIT)
    // But we use lowercase module name for consistency
    Some(BuildToolEntry {
        name: module_name.to_lowercase(),
        command,
        macos,
        linux,
        windows,
        has_custom_installer: has_custom,
    })
}

/// Extract a string value from the block, e.g., `command: "git"` -> "git"
fn extract_string_value(block: &str, key: &str) -> Option<String> {
    let key_pos = block.find(key)?;
    let after_key = &block[key_pos + key.len()..];

    // Find the quoted string
    let quote_start = after_key.find('"')?;
    let remaining = &after_key[quote_start + 1..];
    let quote_end = remaining.find('"')?;

    Some(remaining[..quote_end].to_string())
}

/// Parse the macos block from define_tool!
fn parse_macos_block(block: &str) -> Option<PlatformInstall> {
    let macos_start = block.find("macos:")?;
    let after_macos = &block[macos_start..];
    let brace_start = after_macos.find('{')?;
    let brace_end = after_macos.find('}')?;
    let macos_block = &after_macos[brace_start + 1..brace_end];

    let mut install = PlatformInstall::default();

    if let Some(val) = extract_string_value(macos_block, "brew:") {
        install.brew = Some(val);
    }
    if let Some(val) = extract_string_value(macos_block, "cask:") {
        install.cask = Some(val);
    }

    if install.brew.is_some() || install.cask.is_some() {
        Some(install)
    } else {
        None
    }
}

/// Parse the linux block from define_tool!
fn parse_linux_block(block: &str) -> Option<PlatformInstall> {
    let linux_start = block.find("linux:")?;
    let after_linux = &block[linux_start..];
    let brace_start = after_linux.find('{')?;
    let brace_end = after_linux.find('}')?;
    let linux_block = &after_linux[brace_start + 1..brace_end];

    let mut install = PlatformInstall::default();

    // Check for uniform first
    if let Some(val) = extract_string_value(linux_block, "uniform:") {
        install.uniform = Some(val);
        return Some(install);
    }

    // Check for brew (linuxbrew)
    if let Some(val) = extract_string_value(linux_block, "brew:") {
        install.brew = Some(val);
    }

    // Individual package managers
    if let Some(val) = extract_string_value(linux_block, "apt:") {
        install.apt = Some(val);
    }
    if let Some(val) = extract_string_value(linux_block, "dnf:") {
        install.dnf = Some(val);
    }
    if let Some(val) = extract_string_value(linux_block, "yum:") {
        install.yum = Some(val);
    }
    if let Some(val) = extract_string_value(linux_block, "zypper:") {
        install.zypper = Some(val);
    }
    if let Some(val) = extract_string_value(linux_block, "pacman:") {
        install.pacman = Some(val);
    }
    if let Some(val) = extract_string_value(linux_block, "apk:") {
        install.apk = Some(val);
    }

    let has_any = install.brew.is_some()
        || install.apt.is_some()
        || install.dnf.is_some()
        || install.pacman.is_some()
        || install.apk.is_some();

    if has_any { Some(install) } else { None }
}

/// Parse the windows block from define_tool!
fn parse_windows_block(block: &str) -> Option<PlatformInstall> {
    let windows_start = block.find("windows:")?;
    let after_windows = &block[windows_start..];
    let brace_start = after_windows.find('{')?;
    let brace_end = after_windows.find('}')?;
    let windows_block = &after_windows[brace_start + 1..brace_end];

    let mut install = PlatformInstall::default();

    if let Some(val) = extract_string_value(windows_block, "winget:") {
        install.winget = Some(val);
    }
    if let Some(val) = extract_string_value(windows_block, "choco:") {
        install.choco = Some(val);
    }

    if install.winget.is_some() || install.choco.is_some() {
        Some(install)
    } else {
        None
    }
}

/// Check if this is a custom tool (has add_handler but no define_tool!)
fn is_custom_tool(content: &str) -> bool {
    content.contains("pub fn add_handler") && !content.contains("define_tool!")
}

/// Simple timestamp without external dependencies.
fn chrono_lite_now() -> String {
    // Use build-time environment variable if available, otherwise "unknown"
    env::var("SOURCE_DATE_EPOCH")
        .map(|_| "reproducible-build".to_string())
        .unwrap_or_else(|_| {
            // Just return a placeholder - exact time isn't critical for this index
            "build-time".to_string()
        })
}
