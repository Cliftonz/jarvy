//! Validate jarvy.toml configuration files
//!
//! Checks for:
//! - Syntax errors in TOML
//! - Unknown tool names (with "did you mean?" suggestions)
//! - Invalid version strings
//! - Hook references to undefined tools
//! - Duplicate tool entries

use crate::output::{ExitCode, Outputable, colors, header, icons};
use crate::telemetry;
use crate::tools::spec::{get_tool_dependencies, get_tool_flexible_dependencies, list_tool_names};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Severity level for validation issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARN"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

/// A single validation issue
#[derive(Debug, Clone, Serialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Result of validating a configuration file
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub path: String,
    pub valid: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub issues: Vec<ValidationIssue>,
}

impl Outputable for ValidationResult {
    fn to_human(&self) -> String {
        let mut output = String::new();

        output.push_str(&header(&format!("Validating {}", self.path)));
        output.push('\n');

        if self.issues.is_empty() {
            output.push_str(&format!(
                "\n{}{}{} Configuration is valid!\n",
                colors::GREEN,
                icons::OK,
                colors::RESET
            ));
            return output;
        }

        for issue in &self.issues {
            let (icon, color) = match issue.severity {
                Severity::Error => (icons::ERROR, colors::RED),
                Severity::Warning => (icons::WARN, colors::YELLOW),
                Severity::Info => (icons::INFO, colors::CYAN),
            };

            let line_info = issue
                .line
                .map(|l| format!("Line {}: ", l))
                .unwrap_or_default();

            output.push_str(&format!(
                "{}{}{} {}{}\n",
                color,
                icon,
                colors::RESET,
                line_info,
                issue.message
            ));

            if let Some(ref suggestion) = issue.suggestion {
                output.push_str(&format!(
                    "  {}Suggestion:{} {}\n",
                    colors::DIM,
                    colors::RESET,
                    suggestion
                ));
            }
        }

        output.push_str(&format!(
            "\nValidation {}: {} error(s), {} warning(s)\n",
            if self.valid { "passed" } else { "failed" },
            self.error_count,
            self.warning_count
        ));

        output
    }

    fn exit_code(&self) -> ExitCode {
        if self.error_count > 0 {
            ExitCode::Error
        } else if self.warning_count > 0 {
            ExitCode::Warning
        } else {
            ExitCode::Ok
        }
    }
}

/// Validate a jarvy.toml file
pub fn validate_config(path: &str, strict: bool) -> ValidationResult {
    let mut issues = Vec::new();

    // Check if file exists
    if !Path::new(path).exists() {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            message: format!("Configuration file not found: {}", path),
            line: None,
            suggestion: Some("Create a jarvy.toml file with 'jarvy configure'".to_string()),
        });
        return build_result(path, issues, strict);
    }

    // Read file content
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("Failed to read file: {}", e),
                line: None,
                suggestion: None,
            });
            return build_result(path, issues, strict);
        }
    };

    // Parse TOML
    let parsed: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            let line = extract_line_from_toml_error(&e);
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("Invalid TOML syntax: {}", e),
                line,
                suggestion: Some("Fix the syntax error and try again".to_string()),
            });
            return build_result(path, issues, strict);
        }
    };

    // Validate structure
    validate_structure(&parsed, &content, &mut issues);

    // Validate tools
    if let Some(provisioner) = parsed.get("provisioner") {
        if let Some(tools_table) = provisioner.as_table() {
            validate_tools(tools_table, &mut issues);
        }
    }

    // Validate hooks
    if let Some(hooks) = parsed.get("hooks") {
        if let Some(hooks_table) = hooks.as_table() {
            validate_hooks(hooks_table, &parsed, &mut issues);
        }
    }

    // Validate env
    if let Some(env) = parsed.get("env") {
        if let Some(env_table) = env.as_table() {
            validate_env(env_table, &mut issues);
        }
    }

    // Validate services
    if let Some(services) = parsed.get("services") {
        if let Some(services_table) = services.as_table() {
            validate_services(services_table, &mut issues);
        }
    }

    // Validate package sections — runs the same name/version guards that
    // `jarvy setup` would apply at install time, so `jarvy validate`
    // catches control-byte / flag-like / URL-scheme hostile entries
    // BEFORE the user gets a "configuration is valid" green light on a
    // file they're about to feed to setup.
    for (section, purpose) in [
        ("nuget", "[nuget]"),
        ("npm", "[npm]"),
        ("pip", "[pip]"),
        ("cargo", "[cargo]"),
    ] {
        if let Some(table) = parsed.get(section).and_then(|v| v.as_table()) {
            validate_package_section(table, section, purpose, &mut issues);
        }
    }

    let result = build_result(path, issues, strict);

    // Emit telemetry
    telemetry::validate_result(result.error_count, result.warning_count);

    result
}

fn build_result(path: &str, issues: Vec<ValidationIssue>, strict: bool) -> ValidationResult {
    let error_count = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warning_count = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();

    // In strict mode, warnings become errors
    let valid = if strict {
        error_count == 0 && warning_count == 0
    } else {
        error_count == 0
    };

    ValidationResult {
        path: path.to_string(),
        valid,
        error_count: if strict {
            error_count + warning_count
        } else {
            error_count
        },
        warning_count: if strict { 0 } else { warning_count },
        issues,
    }
}

fn extract_line_from_toml_error(e: &toml::de::Error) -> Option<usize> {
    // TOML errors sometimes include span info
    e.span().map(|_s| {
        // span gives byte offset, we need to convert to line number
        // This is a simplification - in practice we'd count newlines
        1
    })
}

fn validate_structure(parsed: &toml::Value, content: &str, issues: &mut Vec<ValidationIssue>) {
    // Check for required sections
    if parsed.get("provisioner").is_none() {
        issues.push(ValidationIssue {
            severity: Severity::Warning,
            message: "No [provisioner] section found - no tools will be installed".to_string(),
            line: None,
            suggestion: Some("Add a [provisioner] section with tools to install".to_string()),
        });
    }

    // Check for unknown top-level keys. The allowlist lives on
    // `crate::config::TOP_LEVEL_SECTIONS` — a single source of truth shared
    // with `Config`'s field set and pinned by a regression test
    // (`config::tests::top_level_sections_matches_config_fields`). Adding a
    // top-level section in one place without the other will fail to build.
    let known_keys = crate::config::TOP_LEVEL_SECTIONS;
    if let Some(table) = parsed.as_table() {
        // Build the "Valid sections" suggestion once outside the loop —
        // even if many keys are unknown, the suggestion text is constant.
        let valid_sections = known_keys.join(", ");
        for key in table.keys() {
            if !known_keys.contains(&key.as_str()) {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "Unknown configuration section: [{}]",
                        crate::observability::redact_for_display(key)
                    ),
                    line: find_key_line(content, key),
                    suggestion: Some(format!("Valid sections: {}", valid_sections)),
                });
            }
        }
    }
}

fn validate_tools(tools: &toml::map::Map<String, toml::Value>, issues: &mut Vec<ValidationIssue>) {
    let known_tools = list_tool_names();

    // Build a set of configured tool names for dependency checking
    let config_tools: HashSet<String> = tools.keys().map(|k| k.to_lowercase()).collect();

    for (tool_name, value) in tools {
        let tool_lower = tool_name.to_lowercase();

        // Check if tool is known
        if !known_tools.iter().any(|t| t.to_lowercase() == tool_lower) {
            let suggestion = find_similar_tool(&tool_lower, &known_tools);
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("Unknown tool: '{}'", tool_name),
                line: None,
                suggestion,
            });
            continue;
        }

        // Validate version string
        let version = match value {
            toml::Value::String(v) => v.clone(),
            toml::Value::Table(t) => t
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("latest")
                .to_string(),
            _ => {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Invalid value for tool '{}': expected string or table",
                        tool_name
                    ),
                    line: None,
                    suggestion: Some(format!(
                        "{} = \"latest\" or {} = {{ version = \"1.0\" }}",
                        tool_name, tool_name
                    )),
                });
                continue;
            }
        };

        // Validate version format
        if !is_valid_version(&version) {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!(
                    "Version '{}' for tool '{}' may not be valid",
                    version, tool_name
                ),
                line: None,
                suggestion: Some(
                    "Use 'latest', a specific version (1.2.3), or a semver range (>=1.0)"
                        .to_string(),
                ),
            });
        }

        // Validate dependencies
        validate_tool_dependencies(tool_name, &config_tools, issues);
    }
}

/// Validate that a tool's dependencies are configured
fn validate_tool_dependencies(
    tool_name: &str,
    config_tools: &HashSet<String>,
    issues: &mut Vec<ValidationIssue>,
) {
    let strict_deps = get_tool_dependencies(tool_name);
    let flex_deps = get_tool_flexible_dependencies(tool_name);

    // Check strict dependencies (ALL must be in config)
    for dep in strict_deps {
        let dep_lower = dep.to_lowercase();
        if !config_tools.contains(&dep_lower) {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!(
                    "Tool '{}' requires '{}' but it is not in [provisioner]",
                    tool_name, dep
                ),
                line: None,
                suggestion: Some(format!("Add {} = \"latest\" to [provisioner] section", dep)),
            });
        }
    }

    // Check flexible dependencies (at least ONE should be in config)
    if !flex_deps.is_empty() {
        let has_any = flex_deps
            .iter()
            .any(|dep| config_tools.contains(&dep.to_lowercase()));

        if !has_any {
            let options = flex_deps.join(", ");
            let suggestion = flex_deps.first().map(|s| s.to_string());
            issues.push(ValidationIssue {
                severity: Severity::Info,
                message: format!("Tool '{}' works best with one of: {}", tool_name, options),
                line: None,
                suggestion: suggestion
                    .map(|s| format!("Consider adding {} = \"latest\" to [provisioner]", s)),
            });
        }
    }
}

fn validate_hooks(
    hooks: &toml::map::Map<String, toml::Value>,
    full_config: &toml::Value,
    issues: &mut Vec<ValidationIssue>,
) {
    // Get list of configured tools
    let configured_tools: Vec<String> = full_config
        .get("provisioner")
        .and_then(|p| p.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    // Check for tool hooks referencing undefined tools
    for (key, _value) in hooks {
        // Skip known hook config keys
        if ["pre_setup", "post_setup", "config"].contains(&key.as_str()) {
            continue;
        }

        // This should be a tool name
        if !configured_tools
            .iter()
            .any(|t| t.to_lowercase() == key.to_lowercase())
        {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!(
                    "Hook defined for tool '{}' which is not in [provisioner]",
                    key
                ),
                line: None,
                suggestion: Some(format!(
                    "Add {} to [provisioner] or remove [hooks.{}]",
                    key, key
                )),
            });
        }
    }

    // Validate hook config
    if let Some(config) = hooks.get("config") {
        if let Some(config_table) = config.as_table() {
            // Validate shell
            if let Some(shell) = config_table.get("shell") {
                if let Some(shell_str) = shell.as_str() {
                    let valid_shells = ["bash", "zsh", "sh", "fish", "powershell", "pwsh", "cmd"];
                    if !valid_shells.contains(&shell_str.to_lowercase().as_str()) {
                        issues.push(ValidationIssue {
                            severity: Severity::Warning,
                            message: format!("Unknown shell '{}' in hooks.config", shell_str),
                            line: None,
                            suggestion: Some(format!("Valid shells: {}", valid_shells.join(", "))),
                        });
                    }
                }
            }

            // Validate timeout
            if let Some(timeout) = config_table.get("timeout") {
                if let Some(t) = timeout.as_integer() {
                    if t <= 0 {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            message: "Hook timeout must be positive".to_string(),
                            line: None,
                            suggestion: Some(
                                "Use a positive number of seconds (e.g., 300)".to_string(),
                            ),
                        });
                    }
                }
            }
        }
    }
}

fn validate_env(env: &toml::map::Map<String, toml::Value>, issues: &mut Vec<ValidationIssue>) {
    // Validate env var names
    if let Some(vars) = env.get("vars") {
        if let Some(vars_table) = vars.as_table() {
            for (name, _value) in vars_table {
                if !is_valid_env_name(name) {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        message: format!(
                            "Environment variable name '{}' contains invalid characters",
                            name
                        ),
                        line: None,
                        suggestion: Some("Use only letters, numbers, and underscores".to_string()),
                    });
                }
            }
        }
    }

    // Validate secrets config
    if let Some(secrets) = env.get("secrets") {
        if let Some(secrets_table) = secrets.as_table() {
            for (name, _value) in secrets_table {
                if !is_valid_env_name(name) {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        message: format!("Secret name '{}' contains invalid characters", name),
                        line: None,
                        suggestion: Some("Use only letters, numbers, and underscores".to_string()),
                    });
                }
            }
        }
    }
}

fn validate_services(
    services: &toml::map::Map<String, toml::Value>,
    issues: &mut Vec<ValidationIssue>,
) {
    // Check for file paths that may not exist
    if let Some(compose_file) = services.get("compose_file") {
        if let Some(path) = compose_file.as_str() {
            if !Path::new(path).exists() {
                issues.push(ValidationIssue {
                    severity: Severity::Info,
                    message: format!("Compose file '{}' does not exist yet", path),
                    line: None,
                    suggestion: None,
                });
            }
        }
    }

    if let Some(tilt_file) = services.get("tilt_file") {
        if let Some(path) = tilt_file.as_str() {
            if !Path::new(path).exists() {
                issues.push(ValidationIssue {
                    severity: Severity::Info,
                    message: format!("Tiltfile '{}' does not exist yet", path),
                    line: None,
                    suggestion: None,
                });
            }
        }
    }
}

/// Validate every entry in a `[npm]/[pip]/[cargo]/[nuget]` table against
/// the same `validate_package_name` / `validate_package_version` guards
/// that `jarvy setup` applies at install time. Without this pass,
/// `jarvy validate` would print "Configuration is valid!" on a TOML
/// that ships ANSI control bytes in a package name — defeating the
/// safety check operators run on untrusted configs.
///
/// Section-level keys like `package_manager`, `from_lockfile`,
/// `install_dev`, `venv`, `create_venv`, `lockfile`, `activate_hint`,
/// `system_site_packages`, `python_version`, `locked` are NOT package
/// names — they're config knobs flattened into the table by serde.
/// Skip them by exact match.
fn validate_package_section(
    table: &toml::map::Map<String, toml::Value>,
    section: &str,
    purpose: &'static str,
    issues: &mut Vec<ValidationIssue>,
) {
    use crate::packages::common::{validate_package_name, validate_package_version};
    use crate::packages::{CARGO_KNOBS, NPM_KNOBS, NUGET_KNOBS, PIP_KNOBS};

    // Knob lists live on `crate::packages::config` and are pinned by
    // destructure tests against their owning `*Config` struct, so
    // adding a field to e.g. `NugetConfig` without updating
    // `NUGET_KNOBS` fails to compile rather than silently making the
    // validator reject the new knob as a hostile package name.
    let knobs: &[&str] = match section {
        "npm" => NPM_KNOBS,
        "pip" => PIP_KNOBS,
        "cargo" => CARGO_KNOBS,
        "nuget" => NUGET_KNOBS,
        _ => &[],
    };

    for (key, value) in table {
        if knobs.contains(&key.as_str()) {
            continue;
        }
        if let Err(e) = validate_package_name(key, purpose) {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("Refused {} entry: {}", purpose, e),
                line: None,
                suggestion: Some(
                    "Remove the entry or use a name matching the allowed character set."
                        .to_string(),
                ),
            });
            // Skip version check — the name is already poisoned, no
            // point compounding the error message.
            continue;
        }
        // Versions live either as a bare string `name = "1.0"` or as
        // a `{version = "1.0", ...}` inline table.
        let version_str = match value {
            toml::Value::String(s) => Some(s.as_str()),
            toml::Value::Table(t) => t.get("version").and_then(|v| v.as_str()),
            _ => None,
        };
        if let Some(v) = version_str
            && let Err(e) = validate_package_version(v, purpose)
        {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!(
                    "Refused {} version for `{}`: {}",
                    purpose,
                    crate::observability::redact_for_display(key),
                    e
                ),
                line: None,
                suggestion: None,
            });
        }
    }
}

fn find_similar_tool(name: &str, known_tools: &[String]) -> Option<String> {
    let mut best_match: Option<(&str, f64)> = None;

    for tool in known_tools {
        let similarity = strsim::jaro_winkler(name, tool);
        if similarity > 0.6 && (best_match.is_none() || similarity > best_match.unwrap().1) {
            best_match = Some((tool, similarity));
        }
    }

    best_match.map(|(tool, _)| format!("Did you mean '{}'?", tool))
}

fn is_valid_version(version: &str) -> bool {
    use std::sync::LazyLock;

    static SIMPLE_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^\d+(\.\d+)*$").unwrap());
    static SEMVER_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[<>=^~]+\d+(\.\d+)*$").unwrap());
    static RANGE_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^\d+(\.\d+)*\s*-\s*\d+(\.\d+)*$").unwrap());

    // Accept common version formats
    if version == "latest" {
        return true;
    }

    // Toolchain channel aliases used by `rustup` (and conceptually by other
    // version managers). Templates legitimately pin to channels rather than
    // hard versions — `rust = "stable"` is far more common in CI than
    // `rust = "1.80.0"`.
    if matches!(version, "stable" | "beta" | "nightly" | "lts" | "current") {
        return true;
    }

    // Simple version (1, 1.2, 1.2.3)
    if SIMPLE_RE.is_match(version) {
        return true;
    }

    // Semver with operator (>=1.0, ^1.2, ~1.2.3, =1.0.0)
    if SEMVER_RE.is_match(version) {
        return true;
    }

    // Range (1.0 - 2.0)
    if RANGE_RE.is_match(version) {
        return true;
    }

    false
}

fn is_valid_env_name(name: &str) -> bool {
    use std::sync::LazyLock;

    static ENV_NAME_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").unwrap());
    ENV_NAME_RE.is_match(name)
}

fn find_key_line(content: &str, key: &str) -> Option<usize> {
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("[{}]", key))
            || trimmed.starts_with(&format!("{}.", key))
            || trimmed.starts_with(&format!("{} ", key))
            || trimmed.starts_with(&format!("{}=", key))
        {
            return Some(i + 1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_version() {
        assert!(is_valid_version("latest"));
        assert!(is_valid_version("1"));
        assert!(is_valid_version("1.2"));
        assert!(is_valid_version("1.2.3"));
        assert!(is_valid_version(">=1.0"));
        assert!(is_valid_version("^1.2.3"));
        assert!(is_valid_version("~1.2"));
        assert!(is_valid_version("=1.0.0"));
        // Toolchain channels (rust = "stable" etc.)
        assert!(is_valid_version("stable"));
        assert!(is_valid_version("beta"));
        assert!(is_valid_version("nightly"));
        assert!(is_valid_version("lts"));
        assert!(is_valid_version("current"));
    }

    #[test]
    fn test_is_valid_env_name() {
        assert!(is_valid_env_name("MY_VAR"));
        assert!(is_valid_env_name("VAR123"));
        assert!(is_valid_env_name("_PRIVATE"));
        assert!(!is_valid_env_name("123VAR"));
        assert!(!is_valid_env_name("MY-VAR"));
        assert!(!is_valid_env_name("my.var"));
    }

    #[test]
    fn test_find_similar_tool() {
        let tools = vec![
            "git".to_string(),
            "docker".to_string(),
            "node".to_string(),
            "kubernetes".to_string(),
        ];

        // Very similar (one char typo) - should find match
        let suggestion = find_similar_tool("dockers", &tools);
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("docker"));

        // Substring match - kuberntes is very close to kubernetes
        let suggestion = find_similar_tool("kuberntes", &tools);
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("kubernetes"));
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Error.to_string(), "ERROR");
        assert_eq!(Severity::Warning.to_string(), "WARN");
        assert_eq!(Severity::Info.to_string(), "INFO");
    }

    #[test]
    fn test_validation_result_exit_codes() {
        let result_ok = ValidationResult {
            path: "test.toml".to_string(),
            valid: true,
            error_count: 0,
            warning_count: 0,
            issues: vec![],
        };
        assert_eq!(result_ok.exit_code(), ExitCode::Ok);

        let result_warn = ValidationResult {
            path: "test.toml".to_string(),
            valid: true,
            error_count: 0,
            warning_count: 1,
            issues: vec![ValidationIssue {
                severity: Severity::Warning,
                message: "test".to_string(),
                line: None,
                suggestion: None,
            }],
        };
        assert_eq!(result_warn.exit_code(), ExitCode::Warning);

        let result_error = ValidationResult {
            path: "test.toml".to_string(),
            valid: false,
            error_count: 1,
            warning_count: 0,
            issues: vec![ValidationIssue {
                severity: Severity::Error,
                message: "test".to_string(),
                line: None,
                suggestion: None,
            }],
        };
        assert_eq!(result_error.exit_code(), ExitCode::Error);
    }

    #[test]
    fn validate_rejects_control_bytes_in_nuget_name() {
        // A hostile jarvy.toml with a control byte in a [nuget] key
        // must be flagged as an Error — not silently accepted with a
        // green "Configuration is valid!" envelope. TOML's basic string
        // syntax requires escape sequences for control bytes; that's
        // what an attacker would actually write.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            "[provisioner]\ngit = \"latest\"\n\n[nuget]\n\"\\u001b[2J\\u001b[H\" = \"latest\"\n",
        )
        .unwrap();
        let result = validate_config(tmp.path().to_str().unwrap(), false);
        assert!(
            result.issues.iter().any(|i| {
                matches!(i.severity, Severity::Error) && i.message.contains("control bytes")
            }),
            "expected control-byte refusal, got: {:?}",
            result.issues
        );
        assert!(
            result.error_count >= 1,
            "expected non-zero error count, got: {:?}",
            result
        );
    }

    #[test]
    fn validate_rejects_flag_like_npm_name() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            "[provisioner]\ngit = \"latest\"\n\n[npm]\n\"--registry=http://attacker\" = \"latest\"\n",
        ).unwrap();
        let result = validate_config(tmp.path().to_str().unwrap(), false);
        assert!(
            result
                .issues
                .iter()
                .any(|i| matches!(i.severity, Severity::Error)),
            "expected error severity for flag-like npm name: {:?}",
            result.issues
        );
    }

    #[test]
    fn validate_accepts_legitimate_nuget_section() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            "[provisioner]\ngit = \"latest\"\n\n[nuget]\ndotnet-ef = \"latest\"\ncsharpier = \"0.30.0\"\n",
        ).unwrap();
        let result = validate_config(tmp.path().to_str().unwrap(), false);
        assert_eq!(
            result.error_count, 0,
            "legitimate nuget entries rejected: {:?}",
            result.issues
        );
    }

    #[test]
    fn validate_skips_pip_section_knobs_like_venv() {
        // Reserved knobs (venv, create_venv, …) are NOT package names.
        // Don't run validate_package_name on them.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            "[provisioner]\ngit = \"latest\"\n\n[pip]\npytest = \">=7.0\"\nvenv = \".venv\"\ncreate_venv = true\n",
        ).unwrap();
        let result = validate_config(tmp.path().to_str().unwrap(), false);
        assert_eq!(
            result.error_count, 0,
            "pip knobs rejected: {:?}",
            result.issues
        );
    }
}
