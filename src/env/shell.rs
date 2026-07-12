//! Shell rc file modification
//!
//! Updates shell configuration files (.bashrc, .zshrc, fish config) with:
//! - Export statements for environment variables
//! - Jarvy marker comments for easy identification
//! - Backup before modification
//! - Support for bash, zsh, and fish syntax

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::expand::{EnvContext, expand_value};
use crate::telemetry;

/// Errors that can occur during shell rc modification
#[derive(Error, Debug)]
pub enum ShellError {
    #[error("Failed to read shell rc file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Unsupported shell: {0}")]
    UnsupportedShell(String),
    #[error("Failed to backup rc file: {0}")]
    BackupError(String),
    #[error("Could not determine home directory")]
    NoHomeDirectory,
}

/// Supported shell types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    Sh,
    PowerShell,
    Nushell,
}

impl ShellType {
    /// Render one env-var export statement for this shell. The key must
    /// already have passed `is_valid_env_var_name` and the value must
    /// already be quoted via `shell_quote` — this only supplies the
    /// per-shell statement shape. (Replaces the old `export_syntax`
    /// prefix/suffix tuple, which hardcoded `KEY=VALUE` in the middle and
    /// so emitted invalid fish syntax — fish wants `set -gx KEY VALUE` —
    /// and couldn't express nushell's space-mandatory `$env.KEY = VALUE`.)
    pub fn export_line(&self, key: &str, quoted_value: &str) -> String {
        match self {
            ShellType::Fish => format!("set -gx {} {}", key, quoted_value),
            ShellType::PowerShell => format!("$env:{}={}", key, quoted_value),
            ShellType::Nushell => format!("$env.{} = {}", key, quoted_value),
            _ => format!("export {}={}", key, quoted_value),
        }
    }

    /// Get the rc file path for this shell
    pub fn rc_file(&self, home: &Path) -> PathBuf {
        match self {
            ShellType::Bash => home.join(".bashrc"),
            ShellType::Zsh => home.join(".zshrc"),
            ShellType::Fish => home.join(".config/fish/config.fish"),
            ShellType::Sh => home.join(".profile"),
            #[cfg(windows)]
            ShellType::PowerShell => {
                home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1")
            }
            #[cfg(not(windows))]
            ShellType::PowerShell => {
                home.join(".config/powershell/Microsoft.PowerShell_profile.ps1")
            }
            #[cfg(windows)]
            ShellType::Nushell => home.join("AppData/Roaming/nushell/config.nu"),
            #[cfg(not(windows))]
            ShellType::Nushell => home.join(".config/nushell/config.nu"),
        }
    }

    /// Get comment syntax
    pub fn comment_prefix(&self) -> &'static str {
        "#"
    }
}

impl std::fmt::Display for ShellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellType::Bash => write!(f, "bash"),
            ShellType::Zsh => write!(f, "zsh"),
            ShellType::Fish => write!(f, "fish"),
            ShellType::Sh => write!(f, "sh"),
            ShellType::PowerShell => write!(f, "powershell"),
            ShellType::Nushell => write!(f, "nushell"),
        }
    }
}

/// Jarvy marker comments
const JARVY_START: &str = "# >>> jarvy managed start >>>";
const JARVY_END: &str = "# <<< jarvy managed end <<<";

/// Configuration for shell rc modification
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// Whether to backup the rc file before modification
    pub backup: bool,
    /// Whether to validate syntax after modification
    #[allow(dead_code)] // Reserved for shell syntax validation feature
    pub validate: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            backup: true,
            validate: false,
        }
    }
}

/// Detect the current shell type from environment
pub fn detect_shell() -> ShellType {
    if let Ok(shell) = std::env::var("SHELL")
        && let Some(t) = shell_type_from_shell_var(&shell)
    {
        return t;
    }

    // Default to bash on Unix, sh elsewhere
    #[cfg(unix)]
    {
        ShellType::Bash
    }
    #[cfg(not(unix))]
    {
        ShellType::Sh
    }
}

/// Map a `$SHELL` value (path or bare name) to a `ShellType`. Pure —
/// separated from `detect_shell` so the branch logic is table-testable
/// without mutating process-global env state.
fn shell_type_from_shell_var(shell: &str) -> Option<ShellType> {
    let shell_lower = shell.to_lowercase();
    if shell_lower.contains("zsh") {
        Some(ShellType::Zsh)
    } else if shell_lower.contains("bash") {
        Some(ShellType::Bash)
    } else if shell_lower.contains("fish") {
        Some(ShellType::Fish)
    } else if shell_lower.contains("pwsh") || shell_lower.contains("powershell") {
        Some(ShellType::PowerShell)
    } else if shell_lower.ends_with("/nu") || shell_lower == "nu" || shell_lower.contains("nushell")
    {
        Some(ShellType::Nushell)
    } else {
        None
    }
}

/// Parse shell type from string
pub fn parse_shell(s: &str) -> Result<ShellType, ShellError> {
    match s.to_lowercase().as_str() {
        "bash" => Ok(ShellType::Bash),
        "zsh" => Ok(ShellType::Zsh),
        "fish" => Ok(ShellType::Fish),
        "sh" => Ok(ShellType::Sh),
        "powershell" | "pwsh" => Ok(ShellType::PowerShell),
        "nushell" | "nu" => Ok(ShellType::Nushell),
        _ => Err(ShellError::UnsupportedShell(s.to_string())),
    }
}

/// Update shell rc file with environment variables
///
/// # Arguments
/// * `shell` - The shell type to configure
/// * `vars` - HashMap of variable names to their (unexpanded) values
/// * `ctx` - Context for variable expansion
/// * `config` - Configuration for the modification
///
/// # Returns
/// Ok(path) with the path to the modified rc file, or an error
pub fn update_shell_rc(
    shell: ShellType,
    vars: &HashMap<String, String>,
    ctx: &EnvContext,
    config: &ShellConfig,
) -> Result<PathBuf, ShellError> {
    let home = dirs::home_dir().ok_or(ShellError::NoHomeDirectory)?;
    let rc_path = shell.rc_file(&home);

    // Ensure parent directory exists (for fish)
    if let Some(parent) = rc_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Read existing content
    let existing_content = if rc_path.exists() {
        fs::read_to_string(&rc_path)?
    } else {
        String::new()
    };

    // Backup if exists and configured
    if rc_path.exists() && config.backup {
        let backup_path = rc_path.with_extension(format!(
            "{}.jarvy.backup",
            rc_path
                .extension()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default()
        ));
        fs::copy(&rc_path, &backup_path).map_err(|e| {
            ShellError::BackupError(format!(
                "Could not backup {} to {}: {}",
                rc_path.display(),
                backup_path.display(),
                e
            ))
        })?;
    }

    // Generate new content
    let new_content = update_rc_content(&existing_content, shell, vars, ctx);

    // Write the file
    fs::write(&rc_path, new_content)?;

    // Emit telemetry
    telemetry::env_shell_rc_updated(&shell.to_string(), vars.len());

    Ok(rc_path)
}

/// Update rc file content, preserving non-Jarvy content
fn update_rc_content(
    existing: &str,
    shell: ShellType,
    vars: &HashMap<String, String>,
    ctx: &EnvContext,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut in_jarvy_block = false;

    // Process existing content, removing old Jarvy block
    for line in existing.lines() {
        if line.trim() == JARVY_START {
            in_jarvy_block = true;
            continue;
        }
        if line.trim() == JARVY_END {
            in_jarvy_block = false;
            continue;
        }
        if !in_jarvy_block {
            lines.push(line.to_string());
        }
    }

    // Remove trailing empty lines
    while lines.last().map(|s| s.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }

    // Add new Jarvy block if there are variables
    if !vars.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(JARVY_START.to_string());
        lines.push(format!(
            "{} Generated by Jarvy - do not edit manually",
            shell.comment_prefix()
        ));

        // Sort keys for deterministic output
        let mut keys: Vec<_> = vars.keys().collect();
        keys.sort();

        for key in keys {
            // Validate the key matches POSIX env-var grammar before
            // splatting it into a shell rc file. Without this, a hostile
            // `jarvy.toml` shipping `[env.vars] "FOO=x\nrm -rf $HOME #" =
            // "ignored"` would land arbitrary commands inside the
            // `# >>> jarvy managed` block — executed on every shell
            // startup, persistent across reboots (round-2 security P0).
            if !is_valid_env_var_name(key) {
                tracing::warn!(
                    event = "env.refused_invalid_key",
                    key = %key,
                    "refused [env.vars] key that would shell-inject into rc file"
                );
                continue;
            }
            let raw_value = &vars[key];
            let expanded_value = expand_value(raw_value, ctx);
            let quoted_value = shell_quote(&expanded_value, shell);
            lines.push(shell.export_line(key, &quoted_value));
        }

        lines.push(JARVY_END.to_string());
    }

    lines.join("\n") + "\n"
}

/// Validate an env-var name against POSIX-portable grammar:
/// `[A-Za-z_][A-Za-z0-9_]*`. Refuses anything else — newlines, `;`,
/// `=`, leading digits, empty — so a hostile `jarvy.toml` can't
/// shell-inject through the `[env.vars]` key into `~/.bashrc` /
/// `~/.zshrc` / `.env` (round-2 security P0).
pub(crate) fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Quote a value for shell syntax
fn shell_quote(value: &str, shell: ShellType) -> String {
    match shell {
        ShellType::Nushell => {
            // Nushell double-quoted strings escape only `\` and `"`; `$`
            // is literal (interpolation needs the `$"..."` form) and a
            // POSIX-style `\$` would be an *invalid* escape. Always quote
            // — bare words are commands/values with their own parse rules.
            let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        }
        ShellType::Fish => {
            // Fish uses different quoting rules
            if value.contains('\'') {
                // Use double quotes and escape special chars
                let escaped = value
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('$', "\\$");
                format!("\"{}\"", escaped)
            } else if value.contains(' ') || value.contains('$') || value.contains('"') {
                format!("'{}'", value)
            } else {
                value.to_string()
            }
        }
        _ => {
            // Bash/Zsh/Sh quoting
            if value.is_empty()
                || value.contains(' ')
                || value.contains('$')
                || value.contains('`')
                || value.contains('"')
                || value.contains('\'')
                || value.contains('\\')
                || value.contains('#')
            {
                // Use double quotes and escape
                let escaped = value
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('$', "\\$")
                    .replace('`', "\\`");
                format!("\"{}\"", escaped)
            } else {
                value.to_string()
            }
        }
    }
}

/// Preview what would be added to the shell rc file (for dry-run)
pub fn preview_shell_rc(
    shell: ShellType,
    vars: &HashMap<String, String>,
    ctx: &EnvContext,
) -> String {
    let mut lines = Vec::new();
    lines.push(JARVY_START.to_string());

    let mut keys: Vec<_> = vars.keys().collect();
    keys.sort();

    for key in keys {
        if !is_valid_env_var_name(key) {
            // Surface the same refusal in dry-run output so users see what
            // the real run will skip.
            lines.push(format!(
                "{} [refused] invalid env-var name: {}",
                shell.comment_prefix(),
                key
            ));
            continue;
        }
        let raw_value = &vars[key];
        let expanded_value = expand_value(raw_value, ctx);
        let quoted_value = shell_quote(&expanded_value, shell);
        lines.push(shell.export_line(key, &quoted_value));
    }

    lines.push(JARVY_END.to_string());
    lines.join("\n")
}

/// Get the path to the shell rc file for the given shell type
#[allow(dead_code)] // Public API for shell rc path resolution
pub fn get_rc_path(shell: ShellType) -> Result<PathBuf, ShellError> {
    let home = dirs::home_dir().ok_or(ShellError::NoHomeDirectory)?;
    Ok(shell.rc_file(&home))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell() {
        // Just verify it doesn't panic
        let _shell = detect_shell();
    }

    #[test]
    fn shell_var_mapping_covers_every_branch() {
        // Table-tests the pure $SHELL → ShellType mapping so a typo in any
        // pattern (e.g. starts_with instead of ends_with for nu) fails
        // loudly instead of surviving the no-panic smoke test above.
        let cases: &[(&str, Option<ShellType>)] = &[
            ("/bin/zsh", Some(ShellType::Zsh)),
            ("/usr/bin/bash", Some(ShellType::Bash)),
            ("/usr/bin/fish", Some(ShellType::Fish)),
            ("pwsh", Some(ShellType::PowerShell)),
            ("/usr/local/bin/powershell", Some(ShellType::PowerShell)),
            ("/usr/bin/nu", Some(ShellType::Nushell)),
            ("nu", Some(ShellType::Nushell)),
            ("/opt/nushell/nushell", Some(ShellType::Nushell)),
            ("NU", Some(ShellType::Nushell)), // case-insensitive
            ("/bin/csh", None),               // unknown → platform default
            ("/usr/bin/menu", None),          // "nu" substring must NOT match
            ("", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                shell_type_from_shell_var(input),
                *expected,
                "mapping for $SHELL={input:?}"
            );
        }
    }

    #[test]
    fn rc_file_paths_per_shell() {
        let home = std::path::Path::new("/home/u");
        let cases: &[(ShellType, &str)] = &[
            (ShellType::Bash, ".bashrc"),
            (ShellType::Zsh, ".zshrc"),
            (ShellType::Fish, ".config/fish/config.fish"),
            (ShellType::Sh, ".profile"),
            #[cfg(not(windows))]
            (
                ShellType::PowerShell,
                ".config/powershell/Microsoft.PowerShell_profile.ps1",
            ),
            #[cfg(not(windows))]
            (ShellType::Nushell, ".config/nushell/config.nu"),
            #[cfg(windows)]
            (ShellType::Nushell, "AppData/Roaming/nushell/config.nu"),
        ];
        for (shell, suffix) in cases {
            let path = shell.rc_file(home);
            assert!(
                path.ends_with(suffix),
                "{shell} rc_file {path:?} should end with {suffix}"
            );
        }
    }

    #[test]
    fn env_var_name_accepts_posix() {
        for ok in ["FOO", "_BAR", "MY_VAR_1", "X", "_"] {
            assert!(is_valid_env_var_name(ok), "{ok:?} should be valid");
        }
    }

    #[test]
    fn env_var_name_refuses_shell_injection() {
        // Round-2 security P0: a `\n`-bearing key would land RCE in
        // ~/.bashrc / ~/.zshrc on the next interactive shell.
        for hostile in [
            "",
            "1FOO",        // leading digit
            "FOO=BAR",     // embedded `=`
            "FOO\nrm -rf", // newline injection
            "FOO;evil",
            "FOO BAR",  // space
            "FOO\"BAR", // quote
            "FOO'BAR",
            "FOO$VAR",    // expansion
            "FOO`evil`",  // backticks
            "FOO\\nbash", // backslash
            "FOO-BAR",    // hyphen
            "FOO.BAR",    // dot
            "ɛ",          // non-ASCII
        ] {
            assert!(
                !is_valid_env_var_name(hostile),
                "{hostile:?} should be refused"
            );
        }
    }

    #[test]
    fn update_rc_content_skips_invalid_keys() {
        let mut vars = HashMap::new();
        vars.insert("FOO".to_string(), "ok".to_string());
        vars.insert(
            "BAR=evil\nrm -rf $HOME #".to_string(),
            "ignored".to_string(),
        );
        let ctx = EnvContext::new();
        let out = update_rc_content("", ShellType::Bash, &vars, &ctx);
        assert!(out.contains("export FOO="));
        assert!(
            !out.contains("rm -rf"),
            "hostile key must NOT land in rc content; got:\n{out}"
        );
    }

    #[test]
    fn test_parse_shell() {
        assert_eq!(parse_shell("bash").unwrap(), ShellType::Bash);
        assert_eq!(parse_shell("zsh").unwrap(), ShellType::Zsh);
        assert_eq!(parse_shell("fish").unwrap(), ShellType::Fish);
        assert_eq!(parse_shell("BASH").unwrap(), ShellType::Bash);
        assert_eq!(parse_shell("nushell").unwrap(), ShellType::Nushell);
        assert_eq!(parse_shell("nu").unwrap(), ShellType::Nushell);
        assert!(parse_shell("unknown").is_err());
    }

    #[test]
    fn test_shell_export_line() {
        assert_eq!(
            ShellType::Bash.export_line("FOO", "\"v\""),
            "export FOO=\"v\""
        );
        assert_eq!(
            ShellType::Zsh.export_line("FOO", "\"v\""),
            "export FOO=\"v\""
        );
        // Fish assigns with a space, never `=` — `set -gx FOO=v` would set
        // a variable literally named `FOO=v` (pre-export_line bug).
        assert_eq!(ShellType::Fish.export_line("FOO", "'v'"), "set -gx FOO 'v'");
        assert_eq!(
            ShellType::PowerShell.export_line("FOO", "\"v\""),
            "$env:FOO=\"v\""
        );
        // Nushell requires spaces around `=` in assignments.
        assert_eq!(
            ShellType::Nushell.export_line("FOO", "\"v\""),
            "$env.FOO = \"v\""
        );
    }

    #[test]
    fn test_shell_quote_nushell() {
        // Always quoted; `\` and `"` escaped; `$` left literal (nu plain
        // double-quoted strings don't interpolate and `\$` is invalid).
        assert_eq!(shell_quote("plain", ShellType::Nushell), "\"plain\"");
        assert_eq!(
            shell_quote("a \"b\" \\c", ShellType::Nushell),
            "\"a \\\"b\\\" \\\\c\""
        );
        assert_eq!(shell_quote("$HOME", ShellType::Nushell), "\"$HOME\"");
    }

    #[test]
    fn test_shell_quote_simple() {
        assert_eq!(shell_quote("simple", ShellType::Bash), "simple");
    }

    #[test]
    fn test_shell_quote_spaces() {
        assert_eq!(shell_quote("has spaces", ShellType::Bash), "\"has spaces\"");
    }

    #[test]
    fn test_shell_quote_special() {
        assert_eq!(shell_quote("has$var", ShellType::Bash), "\"has\\$var\"");
    }

    #[test]
    fn test_shell_quote_fish() {
        assert_eq!(shell_quote("has spaces", ShellType::Fish), "'has spaces'");
    }

    #[test]
    fn test_update_rc_content_new() {
        let mut vars = HashMap::new();
        vars.insert("MY_VAR".to_string(), "my_value".to_string());

        let ctx = EnvContext::new();
        let result = update_rc_content("", ShellType::Bash, &vars, &ctx);

        assert!(result.contains(JARVY_START));
        assert!(result.contains(JARVY_END));
        assert!(result.contains("export MY_VAR=my_value"));
    }

    #[test]
    fn test_update_rc_content_existing_jarvy_block() {
        let existing = format!(
            "# Some existing config\n{}\nexport OLD_VAR=old\n{}\n# More config",
            JARVY_START, JARVY_END
        );

        let mut vars = HashMap::new();
        vars.insert("NEW_VAR".to_string(), "new_value".to_string());

        let ctx = EnvContext::new();
        let result = update_rc_content(&existing, ShellType::Bash, &vars, &ctx);

        assert!(result.contains("Some existing config"));
        assert!(result.contains("More config"));
        assert!(result.contains("export NEW_VAR=new_value"));
        assert!(!result.contains("OLD_VAR"));
    }

    #[test]
    fn test_update_rc_content_preserve_order() {
        let mut vars = HashMap::new();
        vars.insert("Z_VAR".to_string(), "z".to_string());
        vars.insert("A_VAR".to_string(), "a".to_string());
        vars.insert("M_VAR".to_string(), "m".to_string());

        let ctx = EnvContext::new();
        let result = update_rc_content("", ShellType::Bash, &vars, &ctx);

        let a_pos = result.find("A_VAR").unwrap();
        let m_pos = result.find("M_VAR").unwrap();
        let z_pos = result.find("Z_VAR").unwrap();

        assert!(a_pos < m_pos);
        assert!(m_pos < z_pos);
    }

    #[test]
    fn test_update_rc_content_fish() {
        let mut vars = HashMap::new();
        vars.insert("MY_VAR".to_string(), "my_value".to_string());

        let ctx = EnvContext::new();
        let result = update_rc_content("", ShellType::Fish, &vars, &ctx);

        // Space, not `=` — fish's `set` treats `MY_VAR=my_value` as the
        // variable NAME (the pre-export_line output was broken).
        assert!(result.contains("set -gx MY_VAR my_value"));
    }

    #[test]
    fn test_preview_shell_rc() {
        let mut vars = HashMap::new();
        vars.insert("TEST".to_string(), "value".to_string());

        let ctx = EnvContext::new();
        let preview = preview_shell_rc(ShellType::Bash, &vars, &ctx);

        assert!(preview.contains(JARVY_START));
        assert!(preview.contains("export TEST=value"));
    }
}
