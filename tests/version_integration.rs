//! Integration tests for version checking with real tools.
//!
//! These tests verify that version extraction and comparison work correctly
//! with actual tool output from the system.

use std::process::Command;

/// Helper to get real version output from a command
fn get_version_output(cmd: &str) -> Option<String> {
    Command::new(cmd)
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Helper to check if a command exists on the system
fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

mod version_extraction {
    use super::*;
    use jarvy::tools::version::extract_version;

    #[test]
    fn extract_real_git_version() {
        if !command_exists("git") {
            eprintln!("Skipping: git not installed");
            return;
        }

        let output = get_version_output("git").unwrap();
        println!("Git output: {}", output);

        let version = extract_version(&output);
        assert!(
            version.is_some(),
            "Failed to extract version from: {}",
            output
        );

        let v = version.unwrap();
        println!("Extracted: {}.{}.{}", v.major, v.minor, v.patch);

        // Git versions are typically 2.x
        assert!(v.major >= 1, "Git major version should be >= 1");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn extract_real_brew_version() {
        if !command_exists("brew") {
            eprintln!("Skipping: brew not installed");
            return;
        }

        let output = get_version_output("brew").unwrap();
        println!("Brew output: {}", output);

        let version = extract_version(&output);
        assert!(
            version.is_some(),
            "Failed to extract version from: {}",
            output
        );

        let v = version.unwrap();
        println!("Extracted: {}.{}.{}", v.major, v.minor, v.patch);

        // Homebrew versions are typically 4.x
        assert!(v.major >= 3, "Brew major version should be >= 3");
    }

    #[test]
    fn extract_real_rustc_version() {
        if !command_exists("rustc") {
            eprintln!("Skipping: rustc not installed");
            return;
        }

        let output = get_version_output("rustc").unwrap();
        println!("Rustc output: {}", output);

        let version = extract_version(&output);
        assert!(
            version.is_some(),
            "Failed to extract version from: {}",
            output
        );

        let v = version.unwrap();
        println!("Extracted: {}.{}.{}", v.major, v.minor, v.patch);

        // Rust versions are 1.x
        assert_eq!(v.major, 1, "Rust major version should be 1");
        assert!(v.minor >= 50, "Rust minor version should be >= 50 (modern)");
    }

    #[test]
    fn extract_real_cargo_version() {
        if !command_exists("cargo") {
            eprintln!("Skipping: cargo not installed");
            return;
        }

        let output = get_version_output("cargo").unwrap();
        println!("Cargo output: {}", output);

        let version = extract_version(&output);
        assert!(
            version.is_some(),
            "Failed to extract version from: {}",
            output
        );

        let v = version.unwrap();
        println!("Extracted: {}.{}.{}", v.major, v.minor, v.patch);

        assert_eq!(v.major, 1, "Cargo major version should be 1");
    }

    #[test]
    fn extract_real_node_version() {
        if !command_exists("node") {
            eprintln!("Skipping: node not installed");
            return;
        }

        let output = get_version_output("node").unwrap();
        println!("Node output: {}", output);

        let version = extract_version(&output);
        assert!(
            version.is_some(),
            "Failed to extract version from: {}",
            output
        );

        let v = version.unwrap();
        println!("Extracted: {}.{}.{}", v.major, v.minor, v.patch);

        // Node LTS versions are typically 18, 20, 22+
        assert!(v.major >= 14, "Node major version should be >= 14");
    }

    #[test]
    fn extract_real_python_version() {
        // Try python3 first, then python
        let cmd = if command_exists("python3") {
            "python3"
        } else if command_exists("python") {
            "python"
        } else {
            eprintln!("Skipping: python not installed");
            return;
        };

        let output = get_version_output(cmd).unwrap();
        println!("Python output: {}", output);

        let version = extract_version(&output);
        assert!(
            version.is_some(),
            "Failed to extract version from: {}",
            output
        );

        let v = version.unwrap();
        println!("Extracted: {}.{}.{}", v.major, v.minor, v.patch);

        // Python 3.x
        assert_eq!(v.major, 3, "Python major version should be 3");
        assert!(v.minor >= 8, "Python minor version should be >= 8");
    }

    #[test]
    fn extract_real_docker_version() {
        if !command_exists("docker") {
            eprintln!("Skipping: docker not installed");
            return;
        }

        let output = get_version_output("docker").unwrap();
        println!("Docker output: {}", output);

        let version = extract_version(&output);
        assert!(
            version.is_some(),
            "Failed to extract version from: {}",
            output
        );

        let v = version.unwrap();
        println!("Extracted: {}.{}.{}", v.major, v.minor, v.patch);

        // Docker versions are typically 20+
        assert!(v.major >= 19, "Docker major version should be >= 19");
    }

    #[test]
    fn extract_real_go_version() {
        if !command_exists("go") {
            eprintln!("Skipping: go not installed");
            return;
        }

        // Go uses "go version" not "go --version"
        let output = Command::new("go")
            .arg("version")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

        if output.is_none() {
            eprintln!("Skipping: couldn't get go version");
            return;
        }

        let output = output.unwrap();
        println!("Go output: {}", output);

        let version = extract_version(&output);
        assert!(
            version.is_some(),
            "Failed to extract version from: {}",
            output
        );

        let v = version.unwrap();
        println!("Extracted: {}.{}.{}", v.major, v.minor, v.patch);

        // Go versions are 1.x
        assert_eq!(v.major, 1, "Go major version should be 1");
        assert!(v.minor >= 18, "Go minor version should be >= 18");
    }
}

mod version_satisfies {
    use super::*;
    use jarvy::tools::version::version_satisfies;

    #[test]
    fn real_git_satisfies_latest() {
        if !command_exists("git") {
            return;
        }

        let output = get_version_output("git").unwrap();
        assert!(version_satisfies(&output, "latest"));
        assert!(version_satisfies(&output, "*"));
        assert!(version_satisfies(&output, ""));
    }

    #[test]
    fn real_git_satisfies_major_prefix() {
        if !command_exists("git") {
            return;
        }

        let output = get_version_output("git").unwrap();

        // Git is version 2.x, so "2" should match
        assert!(version_satisfies(&output, "2"), "Git should satisfy '2'");

        // But "1" or "3" should not match
        assert!(
            !version_satisfies(&output, "1"),
            "Git should not satisfy '1'"
        );
        assert!(
            !version_satisfies(&output, "3"),
            "Git should not satisfy '3'"
        );
    }

    #[test]
    fn real_git_satisfies_minimum_version() {
        if !command_exists("git") {
            return;
        }

        let output = get_version_output("git").unwrap();

        // Modern git should be >= 2.0
        assert!(
            version_satisfies(&output, ">= 2.0"),
            "Git should satisfy '>= 2.0'"
        );

        // And definitely >= 1.0
        assert!(
            version_satisfies(&output, ">= 1.0"),
            "Git should satisfy '>= 1.0'"
        );

        // But not >= 99.0
        assert!(
            !version_satisfies(&output, ">= 99.0"),
            "Git should not satisfy '>= 99.0'"
        );
    }

    #[test]
    fn real_rustc_satisfies_caret() {
        if !command_exists("rustc") {
            return;
        }

        let output = get_version_output("rustc").unwrap();

        // Rust 1.x should satisfy ^1.0
        assert!(
            version_satisfies(&output, "^1.0"),
            "Rustc should satisfy '^1.0'"
        );

        // But not ^2.0
        assert!(
            !version_satisfies(&output, "^2.0"),
            "Rustc should not satisfy '^2.0'"
        );
    }

    #[test]
    fn real_node_satisfies_range() {
        if !command_exists("node") {
            return;
        }

        let output = get_version_output("node").unwrap();

        // Modern Node should be >= 14 and < 100
        assert!(
            version_satisfies(&output, ">= 14, < 100"),
            "Node should satisfy '>= 14, < 100'"
        );
    }

    #[test]
    fn real_python_satisfies_major_minor() {
        let cmd = if command_exists("python3") {
            "python3"
        } else if command_exists("python") {
            "python"
        } else {
            return;
        };

        let output = get_version_output(cmd).unwrap();

        // Python 3.x should satisfy "3"
        assert!(version_satisfies(&output, "3"), "Python should satisfy '3'");

        // But not "2"
        assert!(
            !version_satisfies(&output, "2"),
            "Python 3 should not satisfy '2'"
        );
    }
}

mod cmd_satisfies_integration {
    use super::*;
    use jarvy::tools::common::cmd_satisfies;

    #[test]
    fn cmd_satisfies_git_latest() {
        if !command_exists("git") {
            return;
        }

        assert!(cmd_satisfies("git", "latest"));
        assert!(cmd_satisfies("git", "*"));
        assert!(cmd_satisfies("git", ""));
    }

    #[test]
    fn cmd_satisfies_git_version_prefix() {
        if !command_exists("git") {
            return;
        }

        // Git is 2.x
        assert!(cmd_satisfies("git", "2"));
        assert!(!cmd_satisfies("git", "3"));
        assert!(!cmd_satisfies("git", "1"));
    }

    #[test]
    fn cmd_satisfies_rustc_range() {
        if !command_exists("rustc") {
            return;
        }

        // Rust 1.x should satisfy various ranges
        assert!(cmd_satisfies("rustc", ">= 1.50"));
        assert!(cmd_satisfies("rustc", "^1.0"));
        assert!(!cmd_satisfies("rustc", ">= 2.0"));
    }

    #[test]
    fn cmd_satisfies_nonexistent_tool() {
        // A tool that definitely doesn't exist
        assert!(!cmd_satisfies("nonexistent_tool_xyz_123", "1.0"));
    }

    #[test]
    fn cmd_satisfies_no_false_positives() {
        if !command_exists("git") {
            return;
        }

        // Git version is like 2.44.0 - these should NOT match
        assert!(
            !cmd_satisfies("git", "24"),
            "Git 2.44 should not match '24' (false positive check)"
        );
        assert!(
            !cmd_satisfies("git", "44"),
            "Git 2.44 should not match '44' (component check)"
        );
    }
}

/// Test that documents which tools we can successfully parse
#[test]
fn report_parseable_tools() {
    use jarvy::tools::version::extract_version;

    let tools = [
        ("git", "--version"),
        ("rustc", "--version"),
        ("cargo", "--version"),
        ("node", "--version"),
        ("python3", "--version"),
        ("python", "--version"),
        ("docker", "--version"),
        ("brew", "--version"),
        ("go", "version"),
        ("npm", "--version"),
        ("jq", "--version"),
        ("rg", "--version"),
        ("terraform", "--version"),
        ("kubectl", "version --client"),
    ];

    println!("\n=== Tool Version Parsing Report ===\n");

    for (tool, arg) in tools {
        let args: Vec<&str> = arg.split_whitespace().collect();
        let output = Command::new(tool)
            .args(&args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

        match output {
            Some(out) => {
                let first_line = out.lines().next().unwrap_or(&out);
                match extract_version(&out) {
                    Some(v) => {
                        println!(
                            "✓ {:<12} -> {}.{}.{} (from: {})",
                            tool,
                            v.major,
                            v.minor,
                            v.patch,
                            first_line.trim()
                        );
                    }
                    None => {
                        println!(
                            "✗ {:<12} -> PARSE FAILED (output: {})",
                            tool,
                            first_line.trim()
                        );
                    }
                }
            }
            None => {
                println!("- {:<12} -> not installed", tool);
            }
        }
    }

    println!("\n=== End Report ===\n");
}
