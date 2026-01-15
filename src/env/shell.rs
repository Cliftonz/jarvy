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
}

impl ShellType {
    /// Get the export syntax for this shell
    pub fn export_syntax(&self) -> (&'static str, &'static str) {
        match self {
            ShellType::Fish => ("set -gx ", ""),
            _ => ("export ", ""),
        }
    }

    /// Get the rc file path for this shell
    pub fn rc_file(&self, home: &Path) -> PathBuf {
        match self {
            ShellType::Bash => home.join(".bashrc"),
            ShellType::Zsh => home.join(".zshrc"),
            ShellType::Fish => home.join(".config/fish/config.fish"),
            ShellType::Sh => home.join(".profile"),
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
    // Check SHELL environment variable
    if let Ok(shell) = std::env::var("SHELL") {
        let shell_lower = shell.to_lowercase();
        if shell_lower.contains("zsh") {
            return ShellType::Zsh;
        } else if shell_lower.contains("bash") {
            return ShellType::Bash;
        } else if shell_lower.contains("fish") {
            return ShellType::Fish;
        }
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

/// Parse shell type from string
pub fn parse_shell(s: &str) -> Result<ShellType, ShellError> {
    match s.to_lowercase().as_str() {
        "bash" => Ok(ShellType::Bash),
        "zsh" => Ok(ShellType::Zsh),
        "fish" => Ok(ShellType::Fish),
        "sh" => Ok(ShellType::Sh),
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

        let (export_prefix, export_suffix) = shell.export_syntax();

        // Sort keys for deterministic output
        let mut keys: Vec<_> = vars.keys().collect();
        keys.sort();

        for key in keys {
            let raw_value = &vars[key];
            let expanded_value = expand_value(raw_value, ctx);
            let quoted_value = shell_quote(&expanded_value, shell);
            lines.push(format!(
                "{}{}={}{}",
                export_prefix, key, quoted_value, export_suffix
            ));
        }

        lines.push(JARVY_END.to_string());
    }

    lines.join("\n") + "\n"
}

/// Quote a value for shell syntax
fn shell_quote(value: &str, shell: ShellType) -> String {
    match shell {
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

    let (export_prefix, export_suffix) = shell.export_syntax();

    let mut keys: Vec<_> = vars.keys().collect();
    keys.sort();

    for key in keys {
        let raw_value = &vars[key];
        let expanded_value = expand_value(raw_value, ctx);
        let quoted_value = shell_quote(&expanded_value, shell);
        lines.push(format!(
            "{}{}={}{}",
            export_prefix, key, quoted_value, export_suffix
        ));
    }

    lines.push(JARVY_END.to_string());
    lines.join("\n")
}

/// Get the path to the shell rc file for the given shell type
pub fn get_rc_path(shell: ShellType) -> Result<PathBuf, ShellError> {
    let home = dirs::home_dir().ok_or(ShellError::NoHomeDirectory)?;
    Ok(shell.rc_file(&home))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_shell() {
        // Just verify it doesn't panic
        let _shell = detect_shell();
    }

    #[test]
    fn test_parse_shell() {
        assert_eq!(parse_shell("bash").unwrap(), ShellType::Bash);
        assert_eq!(parse_shell("zsh").unwrap(), ShellType::Zsh);
        assert_eq!(parse_shell("fish").unwrap(), ShellType::Fish);
        assert_eq!(parse_shell("BASH").unwrap(), ShellType::Bash);
        assert!(parse_shell("unknown").is_err());
    }

    #[test]
    fn test_shell_export_syntax() {
        assert_eq!(ShellType::Bash.export_syntax(), ("export ", ""));
        assert_eq!(ShellType::Zsh.export_syntax(), ("export ", ""));
        assert_eq!(ShellType::Fish.export_syntax(), ("set -gx ", ""));
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

        assert!(result.contains("set -gx MY_VAR=my_value"));
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
