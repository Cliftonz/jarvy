//! MCP Prompt Handlers
//!
//! Implements the MCP prompt interface for Jarvy:
//! - setup_dev_environment: Interactive workflow to set up a development environment
//! - diagnose_missing_tools: Check which common tools are missing and suggest installations

use crate::mcp::error::{McpError, McpResult};
use serde::Serialize;

/// Prompt definition for MCP prompts/list response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptDefinition {
    /// Prompt name
    pub name: String,
    /// Prompt description
    pub description: String,
    /// Arguments the prompt accepts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Prompt argument definition
#[derive(Debug, Serialize)]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    /// Argument description
    pub description: String,
    /// Whether the argument is required
    pub required: bool,
}

/// List all MCP prompts exposed by Jarvy
pub fn list_prompts() -> Vec<McpPromptDefinition> {
    vec![
        McpPromptDefinition {
            name: "install_jarvy".to_string(),
            description: "Get instructions for installing Jarvy on any platform. Use this to help users who don't have Jarvy installed yet.".to_string(),
            arguments: Some(vec![
                PromptArgument {
                    name: "platform".to_string(),
                    description: "Target platform: 'macos', 'linux', 'windows', or 'auto' to detect".to_string(),
                    required: false,
                },
            ]),
        },
        McpPromptDefinition {
            name: "setup_dev_environment".to_string(),
            description: "Interactive workflow to set up a development environment for a specific project type".to_string(),
            arguments: Some(vec![
                PromptArgument {
                    name: "project_type".to_string(),
                    description: "Type of project (e.g., 'rust', 'node', 'python', 'go', 'java', 'web')".to_string(),
                    required: true,
                },
            ]),
        },
        McpPromptDefinition {
            name: "diagnose_missing_tools".to_string(),
            description: "Check which common development tools are missing and suggest installations".to_string(),
            arguments: None,
        },
    ]
}

/// Get a prompt by name with filled arguments
pub fn get_prompt(
    name: &str,
    arguments: Option<serde_json::Value>,
) -> McpResult<serde_json::Value> {
    match name {
        "install_jarvy" => get_install_jarvy_prompt(arguments),
        "setup_dev_environment" => get_setup_dev_environment_prompt(arguments),
        "diagnose_missing_tools" => get_diagnose_missing_tools_prompt(),
        _ => Err(McpError::method_not_found(format!(
            "Unknown prompt: {}",
            name
        ))),
    }
}

/// Generate the install_jarvy prompt
fn get_install_jarvy_prompt(arguments: Option<serde_json::Value>) -> McpResult<serde_json::Value> {
    let platform = arguments
        .as_ref()
        .and_then(|a| a.get("platform"))
        .and_then(|v| v.as_str())
        .unwrap_or("auto");

    let target_platform = if platform == "auto" {
        #[cfg(target_os = "macos")]
        {
            "macos"
        }
        #[cfg(target_os = "linux")]
        {
            "linux"
        }
        #[cfg(target_os = "windows")]
        {
            "windows"
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            "unknown"
        }
    } else {
        platform
    };

    let prompt_text = match target_platform {
        "macos" => {
            r#"# Installing Jarvy on macOS

Jarvy is a fast, cross-platform CLI for provisioning developer environments. Here's how to install it on macOS:

## Recommended: Quick Install (curl)

```bash
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash
```

This downloads the latest release and installs it to `~/.local/bin`.

## Alternative: Homebrew

```bash
brew install Cliftonz/tap/jarvy
```

## Alternative: Cargo (requires Rust)

```bash
cargo install jarvy
```

## After Installation

1. **Verify installation:**
   ```bash
   jarvy --version
   ```

2. **Create a config file:**
   ```bash
   jarvy init
   ```

3. **Set up your development tools:**
   ```bash
   jarvy setup
   ```

## What is Jarvy?

Jarvy is an open-source tool that helps teams standardize their developer workstations without VMs or containers. Define your tools in a `jarvy.toml` file, and Jarvy installs them using native package managers (Homebrew, apt, winget, etc.).

**Project:** https://github.com/Cliftonz/jarvy
"#
        }
        "linux" => {
            r#"# Installing Jarvy on Linux

Jarvy is a fast, cross-platform CLI for provisioning developer environments. Here's how to install it on Linux:

## Recommended: Quick Install (curl)

```bash
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash
```

This downloads the latest release and installs it to `~/.local/bin`.

## Alternative: Homebrew (if installed)

```bash
brew install Cliftonz/tap/jarvy
```

## Alternative: Cargo (requires Rust)

```bash
cargo install jarvy
```

## After Installation

1. **Verify installation:**
   ```bash
   jarvy --version
   ```

2. **Create a config file:**
   ```bash
   jarvy init
   ```

3. **Set up your development tools:**
   ```bash
   jarvy setup
   ```

## What is Jarvy?

Jarvy is an open-source tool that helps teams standardize their developer workstations without VMs or containers. Define your tools in a `jarvy.toml` file, and Jarvy installs them using native package managers (apt, dnf, pacman, etc.).

**Project:** https://github.com/Cliftonz/jarvy
"#
        }
        "windows" => {
            r#"# Installing Jarvy on Windows

Jarvy is a fast, cross-platform CLI for provisioning developer environments. Here's how to install it on Windows:

## Recommended: Quick Install (PowerShell)

Open PowerShell and run:

```powershell
irm https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.ps1 | iex
```

This downloads the latest release and adds it to your PATH.

## Alternative: winget

```powershell
winget install Jarvy.Jarvy
```

## Alternative: Chocolatey (requires admin)

```powershell
choco install jarvy
```

## Alternative: Cargo (requires Rust)

```powershell
cargo install jarvy
```

## After Installation

1. **Restart your terminal** (to pick up PATH changes)

2. **Verify installation:**
   ```powershell
   jarvy --version
   ```

3. **Create a config file:**
   ```powershell
   jarvy init
   ```

4. **Set up your development tools:**
   ```powershell
   jarvy setup
   ```

## What is Jarvy?

Jarvy is an open-source tool that helps teams standardize their developer workstations without VMs or containers. Define your tools in a `jarvy.toml` file, and Jarvy installs them using native package managers (winget, Chocolatey).

**Project:** https://github.com/Cliftonz/jarvy
"#
        }
        _ => {
            r#"# Installing Jarvy

Jarvy is a fast, cross-platform CLI for provisioning developer environments.

## Universal: Cargo Install

If you have Rust installed:

```bash
cargo install jarvy
```

## Platform-Specific Instructions

Please specify your platform for detailed instructions:
- macOS
- Linux
- Windows

Use the `jarvy_get_install_instructions` tool with your specific platform for detailed commands.

## What is Jarvy?

Jarvy is an open-source tool that helps teams standardize their developer workstations without VMs or containers. Define your tools in a `jarvy.toml` file, and Jarvy installs them using native package managers.

**Project:** https://github.com/Cliftonz/jarvy
"#
        }
    };

    Ok(serde_json::json!({
        "messages": [{
            "role": "user",
            "content": {
                "type": "text",
                "text": prompt_text
            }
        }]
    }))
}

/// Generate the setup_dev_environment prompt
fn get_setup_dev_environment_prompt(
    arguments: Option<serde_json::Value>,
) -> McpResult<serde_json::Value> {
    let project_type = arguments
        .as_ref()
        .and_then(|a| a.get("project_type"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required argument: project_type"))?;

    let tools = get_tools_for_project_type(project_type);
    let tools_list = tools.join(", ");

    let prompt_text = format!(
        r#"# Setting up a {} Development Environment

I'll help you set up a complete development environment for {} development.

## Recommended Tools

The following tools are commonly used for {} development:
{}

## Steps

1. **Check Current Status**
   First, let me check which of these tools are already installed on your system.
   Use `jarvy_check_multiple` with the tools list: [{}]

2. **Review Missing Tools**
   Based on the check results, I'll identify which tools need to be installed.

3. **Install Missing Tools**
   For each missing tool, I'll:
   - Show you the installation command (dry run)
   - Ask for your confirmation before installing
   - Install the tool if confirmed

4. **Verify Installation**
   After installation, I'll verify each tool is working correctly.

## Getting Started

Let me start by checking your current tool status. I'll use Jarvy's `jarvy_check_multiple` tool to see what's already installed.
"#,
        project_type,
        project_type,
        project_type,
        tools
            .iter()
            .map(|t| format!("- {}", t))
            .collect::<Vec<_>>()
            .join("\n"),
        tools_list
    );

    Ok(serde_json::json!({
        "messages": [{
            "role": "user",
            "content": {
                "type": "text",
                "text": prompt_text
            }
        }]
    }))
}

/// Generate the diagnose_missing_tools prompt
fn get_diagnose_missing_tools_prompt() -> McpResult<serde_json::Value> {
    let common_tools = get_common_tools();
    let tools_list = common_tools.join(", ");

    let prompt_text = format!(
        r#"# Diagnosing Missing Development Tools

I'll help you identify which common development tools are missing from your system.

## Common Tools to Check

The following are essential development tools that most developers need:

**Version Control:**
- git - Distributed version control

**Languages & Runtimes:**
- node - JavaScript runtime
- python - Python interpreter
- rust - Rust programming language (via rustup)
- go - Go programming language

**Package Managers:**
- npm/yarn/pnpm - Node.js package managers
- pip - Python package manager
- cargo - Rust package manager

**Containers:**
- docker - Container runtime

**Utilities:**
- jq - JSON processor
- ripgrep - Fast text search
- fd - Fast file finder
- bat - Better cat
- fzf - Fuzzy finder

## Diagnosis Steps

1. **Check All Common Tools**
   Use `jarvy_check_multiple` with: [{}]

2. **Report Status**
   I'll provide a summary of:
   - Tools that are installed and their versions
   - Tools that are missing
   - Recommended installation order

3. **Installation Guidance**
   For missing tools, I'll offer to install them one by one with your confirmation.

Let me start the diagnosis by checking your system.
"#,
        tools_list
    );

    Ok(serde_json::json!({
        "messages": [{
            "role": "user",
            "content": {
                "type": "text",
                "text": prompt_text
            }
        }]
    }))
}

/// Get recommended tools for a project type
fn get_tools_for_project_type(project_type: &str) -> Vec<&'static str> {
    match project_type.to_lowercase().as_str() {
        "rust" => vec!["git", "rust", "cargo"],
        "node" | "nodejs" | "javascript" | "typescript" | "js" | "ts" => {
            vec!["git", "node", "npm"]
        }
        "python" | "py" => vec!["git", "python", "pip"],
        "go" | "golang" => vec!["git", "go"],
        "java" | "kotlin" | "jvm" => vec!["git", "java"],
        "web" | "frontend" => vec!["git", "node", "npm"],
        "devops" | "infrastructure" | "infra" => {
            vec!["git", "docker", "terraform", "kubectl"]
        }
        "data" | "datascience" | "ml" => {
            vec!["git", "python", "pip"]
        }
        _ => vec!["git"], // Default: at least git
    }
}

/// Get list of common development tools
fn get_common_tools() -> Vec<&'static str> {
    vec![
        "git", "node", "python", "rust", "go", "docker", "jq", "ripgrep", "fd", "bat", "fzf",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_prompts() {
        let prompts = list_prompts();
        assert_eq!(prompts.len(), 3);
        assert!(prompts.iter().any(|p| p.name == "install_jarvy"));
        assert!(prompts.iter().any(|p| p.name == "setup_dev_environment"));
        assert!(prompts.iter().any(|p| p.name == "diagnose_missing_tools"));
    }

    #[test]
    fn test_get_install_jarvy_prompt_auto() {
        let result = get_prompt("install_jarvy", None);
        assert!(result.is_ok());
        let prompt = result.unwrap();
        assert!(prompt.get("messages").is_some());
    }

    #[test]
    fn test_get_install_jarvy_prompt_macos() {
        let args = serde_json::json!({"platform": "macos"});
        let result = get_prompt("install_jarvy", Some(args));
        assert!(result.is_ok());
        let prompt = result.unwrap();
        let text = prompt["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("macOS"));
        assert!(text.contains("brew install"));
    }

    #[test]
    fn test_get_install_jarvy_prompt_windows() {
        let args = serde_json::json!({"platform": "windows"});
        let result = get_prompt("install_jarvy", Some(args));
        assert!(result.is_ok());
        let prompt = result.unwrap();
        let text = prompt["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("Windows"));
        assert!(text.contains("winget"));
    }

    #[test]
    fn test_get_setup_dev_environment_prompt() {
        let args = serde_json::json!({"project_type": "rust"});
        let result = get_prompt("setup_dev_environment", Some(args));
        assert!(result.is_ok());
        let prompt = result.unwrap();
        assert!(prompt.get("messages").is_some());
    }

    #[test]
    fn test_get_setup_dev_environment_missing_arg() {
        let result = get_prompt("setup_dev_environment", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_diagnose_missing_tools_prompt() {
        let result = get_prompt("diagnose_missing_tools", None);
        assert!(result.is_ok());
        let prompt = result.unwrap();
        assert!(prompt.get("messages").is_some());
    }

    #[test]
    fn test_unknown_prompt() {
        let result = get_prompt("unknown_prompt", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_tools_for_project_types() {
        let rust_tools = get_tools_for_project_type("rust");
        assert!(rust_tools.contains(&"rust"));
        assert!(rust_tools.contains(&"git"));

        let node_tools = get_tools_for_project_type("node");
        assert!(node_tools.contains(&"node"));
        assert!(node_tools.contains(&"npm"));

        let python_tools = get_tools_for_project_type("python");
        assert!(python_tools.contains(&"python"));
    }
}
