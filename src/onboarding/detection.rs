//! First-run detection and project type detection
//!
//! Detects if this is a first-time user and what type of project
//! is in the current directory.

use std::fs;
use std::path::Path;

/// Marker file name for first-run detection
const FIRST_RUN_MARKER: &str = ".jarvy_initialized";

/// Check if this is the first time Jarvy is being run
///
/// Returns true if the marker file does not exist in the config directory.
pub fn is_first_run() -> bool {
    // Skip first-run detection in CI environments
    if is_ci_environment() {
        return false;
    }

    // Skip in test mode
    if std::env::var("JARVY_TEST_MODE").as_deref() == Ok("1") {
        return false;
    }

    let marker_path = get_marker_path();
    match marker_path {
        Some(path) => !path.exists(),
        None => false, // Can't determine, assume not first run
    }
}

/// Mark Jarvy as initialized (first run complete)
///
/// Creates the marker file in the config directory.
pub fn mark_initialized() -> Result<(), std::io::Error> {
    let marker_path = get_marker_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Could not determine config directory")
    })?;

    // Ensure parent directory exists
    if let Some(parent) = marker_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(marker_path, "")?;
    Ok(())
}

/// Get the path to the first-run marker file
fn get_marker_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".jarvy").join(FIRST_RUN_MARKER))
}

/// Check if we're running in a CI environment
fn is_ci_environment() -> bool {
    std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("GITLAB_CI").is_ok()
        || std::env::var("CIRCLECI").is_ok()
        || std::env::var("TRAVIS").is_ok()
        || std::env::var("JENKINS_URL").is_ok()
        || std::env::var("BUILDKITE").is_ok()
}

/// Type of project detected in the current directory
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectType {
    /// Node.js / JavaScript project (package.json)
    NodeJs,
    /// Rust project (Cargo.toml)
    Rust,
    /// Go project (go.mod)
    Go,
    /// Python project (requirements.txt, pyproject.toml, setup.py)
    Python,
    /// Java project (pom.xml, build.gradle)
    Java,
    /// Ruby project (Gemfile)
    Ruby,
    /// .NET / C# project (*.csproj, *.sln)
    DotNet,
    /// PHP project (composer.json)
    Php,
    /// Elixir project (mix.exs)
    Elixir,
    /// Flutter / Dart project (pubspec.yaml)
    Flutter,
    /// Terraform project (*.tf files)
    Terraform,
    /// Docker project (Dockerfile, docker-compose.yml)
    Docker,
    /// Kubernetes project (k8s manifests)
    Kubernetes,
    /// Unknown or no specific project type detected
    Unknown,
}

impl ProjectType {
    /// Get a human-readable name for the project type
    pub fn display_name(&self) -> &'static str {
        match self {
            ProjectType::NodeJs => "Node.js",
            ProjectType::Rust => "Rust",
            ProjectType::Go => "Go",
            ProjectType::Python => "Python",
            ProjectType::Java => "Java",
            ProjectType::Ruby => "Ruby",
            ProjectType::DotNet => ".NET",
            ProjectType::Php => "PHP",
            ProjectType::Elixir => "Elixir",
            ProjectType::Flutter => "Flutter",
            ProjectType::Terraform => "Terraform",
            ProjectType::Docker => "Docker",
            ProjectType::Kubernetes => "Kubernetes",
            ProjectType::Unknown => "Unknown",
        }
    }

    /// Get suggested template name for this project type
    pub fn suggested_template(&self) -> Option<&'static str> {
        match self {
            ProjectType::NodeJs => Some("react"), // Default to React for Node.js
            ProjectType::Rust => Some("rust-cli"),
            ProjectType::Go => Some("go-api"),
            ProjectType::Python => Some("python-api"),
            ProjectType::Java => Some("java-spring"),
            ProjectType::Ruby => None, // No Ruby template yet
            ProjectType::DotNet => None,
            ProjectType::Php => None,
            ProjectType::Elixir => None,
            ProjectType::Flutter => Some("flutter"),
            ProjectType::Terraform => Some("terraform"),
            ProjectType::Docker => Some("docker-dev"),
            ProjectType::Kubernetes => Some("k8s-admin"),
            ProjectType::Unknown => Some("essential"),
        }
    }
}

/// Result of project type detection
#[derive(Debug, Clone)]
pub struct DetectedProject {
    /// Primary project type detected
    pub primary: ProjectType,
    /// All project types detected (for multi-stack projects)
    pub all: Vec<ProjectType>,
    /// Files that were used for detection
    pub detection_files: Vec<String>,
}

impl DetectedProject {
    /// Check if this is a multi-stack project
    pub fn is_multi_stack(&self) -> bool {
        self.all.len() > 1
    }
}

/// Detect the project type in the given directory
///
/// Scans for common project files and returns the detected project type(s).
pub fn detect_project_type<P: AsRef<Path>>(dir: P) -> DetectedProject {
    let dir = dir.as_ref();
    let mut detected = Vec::new();
    let mut detection_files = Vec::new();

    // Node.js detection
    if dir.join("package.json").exists() {
        detected.push(ProjectType::NodeJs);
        detection_files.push("package.json".to_string());
    }

    // Rust detection
    if dir.join("Cargo.toml").exists() {
        detected.push(ProjectType::Rust);
        detection_files.push("Cargo.toml".to_string());
    }

    // Go detection
    if dir.join("go.mod").exists() {
        detected.push(ProjectType::Go);
        detection_files.push("go.mod".to_string());
    }

    // Python detection
    if dir.join("pyproject.toml").exists() {
        detected.push(ProjectType::Python);
        detection_files.push("pyproject.toml".to_string());
    } else if dir.join("requirements.txt").exists() {
        detected.push(ProjectType::Python);
        detection_files.push("requirements.txt".to_string());
    } else if dir.join("setup.py").exists() {
        detected.push(ProjectType::Python);
        detection_files.push("setup.py".to_string());
    }

    // Java detection
    if dir.join("pom.xml").exists() {
        detected.push(ProjectType::Java);
        detection_files.push("pom.xml".to_string());
    } else if dir.join("build.gradle").exists() || dir.join("build.gradle.kts").exists() {
        detected.push(ProjectType::Java);
        if dir.join("build.gradle").exists() {
            detection_files.push("build.gradle".to_string());
        } else {
            detection_files.push("build.gradle.kts".to_string());
        }
    }

    // Ruby detection
    if dir.join("Gemfile").exists() {
        detected.push(ProjectType::Ruby);
        detection_files.push("Gemfile".to_string());
    }

    // .NET detection
    if has_file_with_extension(dir, "csproj") || has_file_with_extension(dir, "sln") {
        detected.push(ProjectType::DotNet);
        detection_files.push("*.csproj/*.sln".to_string());
    }

    // PHP detection
    if dir.join("composer.json").exists() {
        detected.push(ProjectType::Php);
        detection_files.push("composer.json".to_string());
    }

    // Elixir detection
    if dir.join("mix.exs").exists() {
        detected.push(ProjectType::Elixir);
        detection_files.push("mix.exs".to_string());
    }

    // Flutter/Dart detection
    if dir.join("pubspec.yaml").exists() {
        detected.push(ProjectType::Flutter);
        detection_files.push("pubspec.yaml".to_string());
    }

    // Terraform detection
    if has_file_with_extension(dir, "tf") {
        detected.push(ProjectType::Terraform);
        detection_files.push("*.tf".to_string());
    }

    // Docker detection
    if dir.join("Dockerfile").exists() || dir.join("docker-compose.yml").exists() || dir.join("docker-compose.yaml").exists() {
        detected.push(ProjectType::Docker);
        if dir.join("Dockerfile").exists() {
            detection_files.push("Dockerfile".to_string());
        }
        if dir.join("docker-compose.yml").exists() {
            detection_files.push("docker-compose.yml".to_string());
        }
    }

    // Kubernetes detection (look for k8s directory or common manifest patterns)
    if dir.join("k8s").is_dir() || dir.join("kubernetes").is_dir() || dir.join("manifests").is_dir() {
        detected.push(ProjectType::Kubernetes);
        detection_files.push("k8s/".to_string());
    }

    // Determine primary type (first detected, or Unknown if none)
    let primary = detected.first().cloned().unwrap_or(ProjectType::Unknown);

    DetectedProject {
        primary,
        all: if detected.is_empty() { vec![ProjectType::Unknown] } else { detected },
        detection_files,
    }
}

/// Check if a directory contains any file with the given extension
fn has_file_with_extension(dir: &Path, extension: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == extension {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_detect_nodejs_project() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("package.json")).unwrap();

        let detected = detect_project_type(dir.path());
        assert_eq!(detected.primary, ProjectType::NodeJs);
        assert!(detected.detection_files.contains(&"package.json".to_string()));
    }

    #[test]
    fn test_detect_rust_project() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("Cargo.toml")).unwrap();

        let detected = detect_project_type(dir.path());
        assert_eq!(detected.primary, ProjectType::Rust);
    }

    #[test]
    fn test_detect_go_project() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("go.mod")).unwrap();

        let detected = detect_project_type(dir.path());
        assert_eq!(detected.primary, ProjectType::Go);
    }

    #[test]
    fn test_detect_python_project() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("pyproject.toml")).unwrap();

        let detected = detect_project_type(dir.path());
        assert_eq!(detected.primary, ProjectType::Python);
    }

    #[test]
    fn test_detect_multi_stack_project() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("package.json")).unwrap();
        File::create(dir.path().join("Dockerfile")).unwrap();

        let detected = detect_project_type(dir.path());
        assert!(detected.is_multi_stack());
        assert!(detected.all.contains(&ProjectType::NodeJs));
        assert!(detected.all.contains(&ProjectType::Docker));
    }

    #[test]
    fn test_detect_unknown_project() {
        let dir = tempdir().unwrap();
        // Empty directory

        let detected = detect_project_type(dir.path());
        assert_eq!(detected.primary, ProjectType::Unknown);
    }

    #[test]
    fn test_project_type_display_name() {
        assert_eq!(ProjectType::NodeJs.display_name(), "Node.js");
        assert_eq!(ProjectType::Rust.display_name(), "Rust");
        assert_eq!(ProjectType::Go.display_name(), "Go");
    }

    #[test]
    fn test_project_type_suggested_template() {
        assert_eq!(ProjectType::Rust.suggested_template(), Some("rust-cli"));
        assert_eq!(ProjectType::Go.suggested_template(), Some("go-api"));
        assert_eq!(ProjectType::Unknown.suggested_template(), Some("essential"));
    }

    #[test]
    fn test_is_ci_environment() {
        // This test just verifies the function doesn't panic
        // Actual CI detection depends on environment
        let _ = is_ci_environment();
    }
}
