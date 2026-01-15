//! Hook execution module for running shell scripts before/after tool installation.
//!
//! Supports bash, zsh, sh on Unix and PowerShell on Windows.
//! Includes timeout support and environment variable injection.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;
use thiserror::Error;
use wait_timeout::ChildExt;

use crate::config::{DEFAULT_HOOK_TIMEOUT, HookSettings};
use crate::tools::Os;

/// Errors that can occur during hook execution
#[derive(Error, Debug)]
pub enum HookError {
    #[error("Hook timed out after {0} seconds")]
    Timeout(u64),

    #[error("Hook failed with exit code {0}: {1}")]
    Failed(i32, String),

    #[error("Hook process was terminated by signal")]
    Terminated,

    #[error("Failed to spawn hook process: {0}")]
    SpawnError(#[from] std::io::Error),

    #[error("Shell not found: {0}")]
    ShellNotFound(String),
}

/// Result type for hook operations
pub type HookResult<T> = Result<T, HookError>;

/// Configuration for a single hook execution
#[derive(Debug, Clone)]
pub struct HookConfig {
    /// Shell to use for execution
    pub shell: String,
    /// Timeout in seconds
    pub timeout: u64,
    /// Continue setup if hook fails
    pub continue_on_error: bool,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            shell: detect_shell(),
            timeout: DEFAULT_HOOK_TIMEOUT,
            continue_on_error: false,
        }
    }
}

impl From<&HookSettings> for HookConfig {
    fn from(settings: &HookSettings) -> Self {
        Self {
            shell: settings.shell.clone(),
            timeout: settings.timeout,
            continue_on_error: settings.continue_on_error,
        }
    }
}

/// Environment variables to inject into hook scripts
#[derive(Debug, Clone, Default)]
pub struct HookEnv {
    /// Tool name (e.g., "node", "rust")
    pub tool: Option<String>,
    /// Installed version
    pub version: Option<String>,
    /// Operating system
    pub os: Option<Os>,
    /// CPU architecture
    pub arch: Option<String>,
    /// Jarvy home directory
    pub jarvy_home: Option<String>,
    /// Additional custom environment variables
    pub custom: HashMap<String, String>,
}

impl HookEnv {
    /// Create environment for a specific tool
    pub fn for_tool(name: &str, version: &str) -> Self {
        Self {
            tool: Some(name.to_string()),
            version: Some(version.to_string()),
            os: Some(crate::tools::current_os()),
            arch: Some(std::env::consts::ARCH.to_string()),
            jarvy_home: dirs::home_dir().map(|p| p.join(".jarvy").to_string_lossy().to_string()),
            custom: HashMap::new(),
        }
    }

    /// Create environment for global hooks (pre_setup, post_setup)
    pub fn global() -> Self {
        Self {
            tool: None,
            version: None,
            os: Some(crate::tools::current_os()),
            arch: Some(std::env::consts::ARCH.to_string()),
            jarvy_home: dirs::home_dir().map(|p| p.join(".jarvy").to_string_lossy().to_string()),
            custom: HashMap::new(),
        }
    }

    /// Add a custom environment variable
    pub fn with_var(mut self, key: &str, value: &str) -> Self {
        self.custom.insert(key.to_string(), value.to_string());
        self
    }

    /// Convert to a HashMap for Command::envs()
    fn to_env_map(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if let Some(ref tool) = self.tool {
            env.insert("JARVY_TOOL".to_string(), tool.clone());
        }
        if let Some(ref version) = self.version {
            env.insert("JARVY_VERSION".to_string(), version.clone());
        }
        if let Some(ref os) = self.os {
            env.insert("JARVY_OS".to_string(), format!("{:?}", os).to_lowercase());
        }
        if let Some(ref arch) = self.arch {
            env.insert("JARVY_ARCH".to_string(), arch.clone());
        }
        if let Some(ref home) = self.jarvy_home {
            env.insert("JARVY_HOME".to_string(), home.clone());
        }

        // Add custom vars
        for (k, v) in &self.custom {
            env.insert(k.clone(), v.clone());
        }

        env
    }
}

/// A hook to be executed
#[derive(Debug, Clone)]
pub struct Hook {
    /// The script content to execute
    pub script: String,
    /// Configuration for execution
    pub config: HookConfig,
    /// Environment variables to inject
    pub env: HookEnv,
    /// Description for logging
    pub description: String,
}

impl Hook {
    /// Create a new hook with default configuration
    pub fn new(script: &str, description: &str) -> Self {
        Self {
            script: script.to_string(),
            config: HookConfig::default(),
            env: HookEnv::global(),
            description: description.to_string(),
        }
    }

    /// Create a hook with custom configuration
    pub fn with_config(script: &str, description: &str, config: HookConfig) -> Self {
        Self {
            script: script.to_string(),
            config,
            env: HookEnv::global(),
            description: description.to_string(),
        }
    }

    /// Set environment variables for the hook
    pub fn with_env(mut self, env: HookEnv) -> Self {
        self.env = env;
        self
    }

    /// Execute the hook script
    ///
    /// Returns Ok(output) on success, or HookError on failure.
    /// Output is streamed to stdout/stderr in real-time.
    pub fn execute(&self) -> HookResult<String> {
        println!("  Running hook: {}", self.description);

        let (shell, args) = build_shell_command(&self.config.shell, &self.script)?;

        let mut child = Command::new(&shell)
            .args(&args)
            .envs(self.env.to_env_map())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Capture stdout in a separate thread
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_handle = std::thread::spawn(move || {
            let mut output = String::new();
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    println!("    {}", line);
                    output.push_str(&line);
                    output.push('\n');
                }
            }
            output
        });

        let stderr_handle = std::thread::spawn(move || {
            let mut output = String::new();
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("    {}", line);
                    output.push_str(&line);
                    output.push('\n');
                }
            }
            output
        });

        // Wait with timeout
        let timeout = Duration::from_secs(self.config.timeout);
        let status = match child.wait_timeout(timeout)? {
            Some(status) => status,
            None => {
                // Timeout - kill the process
                let _ = child.kill();
                let _ = child.wait();
                return Err(HookError::Timeout(self.config.timeout));
            }
        };

        // Wait for output threads
        let stdout_output = stdout_handle.join().unwrap_or_default();
        let stderr_output = stderr_handle.join().unwrap_or_default();

        if status.success() {
            println!("  Hook completed successfully");
            Ok(stdout_output)
        } else {
            let code = status.code().unwrap_or(-1);
            if code == -1 {
                Err(HookError::Terminated)
            } else {
                Err(HookError::Failed(code, stderr_output))
            }
        }
    }

    /// Execute in dry-run mode (just print what would happen)
    pub fn dry_run(&self) {
        println!("  [DRY-RUN] Would run hook: {}", self.description);
        println!("    Shell: {}", self.config.shell);
        println!("    Timeout: {}s", self.config.timeout);
        println!("    Continue on error: {}", self.config.continue_on_error);
        println!("    Script:");
        for line in self.script.lines() {
            println!("      {}", line);
        }
        if !self.env.to_env_map().is_empty() {
            println!("    Environment:");
            for (k, v) in self.env.to_env_map() {
                println!("      {}={}", k, v);
            }
        }
    }
}

/// Detect the default shell for the current platform
pub fn detect_shell() -> String {
    #[cfg(windows)]
    {
        "powershell".to_string()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

/// Build the shell command and arguments for script execution
fn build_shell_command(shell: &str, script: &str) -> HookResult<(String, Vec<String>)> {
    let shell_lower = shell.to_lowercase();

    // Determine the shell binary and args based on the shell name
    let (shell_bin, args) = if shell_lower.contains("powershell") || shell_lower == "pwsh" {
        // PowerShell
        let bin = if cfg!(windows) {
            "powershell.exe"
        } else {
            "pwsh" // PowerShell Core on Unix
        };
        (
            bin.to_string(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                script.to_string(),
            ],
        )
    } else if shell_lower.contains("cmd") {
        // Windows CMD
        (
            "cmd.exe".to_string(),
            vec!["/C".to_string(), script.to_string()],
        )
    } else {
        // Unix shells (bash, zsh, sh, fish, etc.)
        let shell_path = if shell.starts_with('/') {
            shell.to_string()
        } else {
            // Try to find the shell in common locations
            let paths = [
                format!("/bin/{}", shell),
                format!("/usr/bin/{}", shell),
                format!("/usr/local/bin/{}", shell),
            ];
            paths
                .iter()
                .find(|p| std::path::Path::new(p).exists())
                .cloned()
                .unwrap_or_else(|| shell.to_string())
        };

        // Fish has different syntax
        if shell_lower == "fish" {
            (shell_path, vec!["-c".to_string(), script.to_string()])
        } else {
            // bash, zsh, sh, etc.
            (shell_path, vec!["-c".to_string(), script.to_string()])
        }
    };

    Ok((shell_bin, args))
}

/// Execute a list of hooks, respecting continue_on_error settings
pub fn execute_hooks(hooks: &[Hook], dry_run: bool) -> HookResult<()> {
    for hook in hooks {
        if dry_run {
            hook.dry_run();
            continue;
        }

        match hook.execute() {
            Ok(_) => {}
            Err(e) => {
                if hook.config.continue_on_error {
                    eprintln!("  Warning: Hook failed but continuing: {}", e);
                } else {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell() {
        let shell = detect_shell();
        assert!(!shell.is_empty());
        #[cfg(windows)]
        assert_eq!(shell, "powershell");
    }

    #[test]
    fn test_hook_config_default() {
        let config = HookConfig::default();
        assert_eq!(config.timeout, DEFAULT_HOOK_TIMEOUT);
        assert!(!config.continue_on_error);
    }

    #[test]
    fn test_hook_env_for_tool() {
        let env = HookEnv::for_tool("node", "20.0.0");
        let map = env.to_env_map();
        assert_eq!(map.get("JARVY_TOOL"), Some(&"node".to_string()));
        assert_eq!(map.get("JARVY_VERSION"), Some(&"20.0.0".to_string()));
        assert!(map.contains_key("JARVY_OS"));
        assert!(map.contains_key("JARVY_ARCH"));
    }

    #[test]
    fn test_hook_env_global() {
        let env = HookEnv::global();
        let map = env.to_env_map();
        assert!(!map.contains_key("JARVY_TOOL"));
        assert!(!map.contains_key("JARVY_VERSION"));
        assert!(map.contains_key("JARVY_OS"));
        assert!(map.contains_key("JARVY_ARCH"));
    }

    #[test]
    fn test_hook_env_custom() {
        let env = HookEnv::global().with_var("MY_VAR", "my_value");
        let map = env.to_env_map();
        assert_eq!(map.get("MY_VAR"), Some(&"my_value".to_string()));
    }

    #[test]
    fn test_build_shell_command_bash() {
        let (shell, args) = build_shell_command("bash", "echo hello").unwrap();
        assert!(shell.contains("bash") || shell == "bash");
        assert_eq!(args, vec!["-c", "echo hello"]);
    }

    #[test]
    fn test_build_shell_command_sh() {
        let (shell, args) = build_shell_command("/bin/sh", "echo hello").unwrap();
        assert_eq!(shell, "/bin/sh");
        assert_eq!(args, vec!["-c", "echo hello"]);
    }

    #[test]
    fn test_build_shell_command_powershell() {
        let (shell, args) = build_shell_command("powershell", "Write-Host hello").unwrap();
        #[cfg(windows)]
        assert_eq!(shell, "powershell.exe");
        #[cfg(not(windows))]
        assert_eq!(shell, "pwsh");
        assert!(args.contains(&"-Command".to_string()));
    }

    #[test]
    fn test_hook_creation() {
        let hook = Hook::new("echo test", "Test hook");
        assert_eq!(hook.script, "echo test");
        assert_eq!(hook.description, "Test hook");
    }

    #[test]
    fn test_hook_with_env() {
        let hook =
            Hook::new("echo $JARVY_TOOL", "Tool hook").with_env(HookEnv::for_tool("git", "2.40.0"));
        let map = hook.env.to_env_map();
        assert_eq!(map.get("JARVY_TOOL"), Some(&"git".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn test_hook_execute_simple() {
        let hook = Hook::new("echo 'hello from hook'", "Simple echo");
        let result = hook.execute();
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello from hook"));
    }

    #[test]
    #[cfg(unix)]
    fn test_hook_execute_with_env() {
        let hook = Hook::new("echo $JARVY_TOOL", "Echo tool name")
            .with_env(HookEnv::for_tool("testool", "1.0.0"));
        let result = hook.execute();
        assert!(result.is_ok());
        assert!(result.unwrap().contains("testool"));
    }

    #[test]
    #[cfg(unix)]
    fn test_hook_execute_failure() {
        let hook = Hook::new("exit 1", "Failing hook");
        let result = hook.execute();
        assert!(result.is_err());
        match result {
            Err(HookError::Failed(code, _)) => assert_eq!(code, 1),
            _ => panic!("Expected Failed error"),
        }
    }

    #[test]
    fn test_hook_dry_run() {
        // Just ensure it doesn't panic
        let hook = Hook::new("echo test", "Dry run test");
        hook.dry_run();
    }
}
