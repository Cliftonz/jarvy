//! Built-in templates for common development stacks
//!
//! These templates are embedded in the binary and available offline.

#![allow(dead_code)] // Public API for template management

use super::schema::{Template, TemplateMeta, TemplateTools};
use std::collections::HashMap;

/// A built-in template with metadata
#[derive(Debug, Clone)]
pub struct BuiltinTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub tools: &'static [(&'static str, &'static str)],
}

impl BuiltinTemplate {
    /// Convert to a full Template struct
    pub fn to_template(&self) -> Template {
        let mut tools = TemplateTools::new();
        for (name, version) in self.tools {
            tools.add(name, version);
        }

        Template {
            template: TemplateMeta {
                name: self.name.to_string(),
                description: self.description.to_string(),
                category: self.category.to_string(),
                tags: vec![],
                author: Some("Jarvy Team".to_string()),
                version: Some("1.0.0".to_string()),
                min_jarvy_version: Some("0.1.0".to_string()),
            },
            tools,
            hooks: HashMap::new(),
        }
    }
}

/// All built-in templates
pub const BUILTIN_TEMPLATES: &[BuiltinTemplate] = &[
    // Web Development
    BuiltinTemplate {
        name: "react",
        description: "Complete React development environment with modern tooling",
        category: "Web Development",
        tools: &[
            ("node", "20"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("bat", "latest"),
            ("eza", "latest"),
            ("fzf", "latest"),
            ("starship", "latest"),
            ("gh", "latest"),
            ("httpie", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "vue",
        description: "Vue.js development stack with Vite tooling",
        category: "Web Development",
        tools: &[
            ("node", "20"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("bat", "latest"),
            ("fzf", "latest"),
            ("gh", "latest"),
            ("httpie", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "nextjs",
        description: "Next.js full-stack template with deployment tooling",
        category: "Web Development",
        tools: &[
            ("node", "20"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("bat", "latest"),
            ("eza", "latest"),
            ("fzf", "latest"),
            ("starship", "latest"),
            ("gh", "latest"),
            ("httpie", "latest"),
            ("awscli", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "angular",
        description: "Angular development environment with TypeScript tooling",
        category: "Web Development",
        tools: &[
            ("node", "20"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("bat", "latest"),
            ("fzf", "latest"),
            ("gh", "latest"),
            ("httpie", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "svelte",
        description: "Svelte/SvelteKit stack with Vite and modern tooling",
        category: "Web Development",
        tools: &[
            ("node", "20"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("bat", "latest"),
            ("fzf", "latest"),
            ("gh", "latest"),
            ("httpie", "latest"),
        ],
    },
    // Backend
    BuiltinTemplate {
        name: "node-api",
        description: "Node.js API development with Express/Fastify patterns",
        category: "Backend",
        tools: &[
            ("node", "20"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("httpie", "latest"),
            ("gh", "latest"),
            ("redis", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "go-api",
        description: "Go backend development with common CLI tools",
        category: "Backend",
        tools: &[
            ("go", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("httpie", "latest"),
            ("gh", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "rust-cli",
        description: "Rust CLI development with cargo extensions",
        category: "Backend",
        tools: &[
            ("rust", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("gh", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "python-api",
        description: "Python/FastAPI development with virtual environment tools",
        category: "Backend",
        tools: &[
            ("python", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("httpie", "latest"),
            ("gh", "latest"),
            ("redis", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "java-spring",
        description: "Java Spring Boot development with Maven/Gradle",
        category: "Backend",
        tools: &[
            ("java", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("httpie", "latest"),
            ("gh", "latest"),
            ("redis", "latest"),
        ],
    },
    // .NET
    BuiltinTemplate {
        name: "dotnet-api",
        description: "ASP.NET Core Web API with EF Core migrations and the standard tool chain",
        category: "Backend",
        tools: &[
            ("dotnet", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("httpie", "latest"),
            ("gh", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "dotnet-console",
        description: "Minimal .NET console application toolkit (CLI / cron / one-shot job)",
        category: "Backend",
        tools: &[
            ("dotnet", "latest"),
            ("git", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("gh", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "dotnet-worker",
        description: ".NET Worker Service (BackgroundService) for queues, schedulers, and daemons",
        category: "Backend",
        tools: &[
            ("dotnet", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("gh", "latest"),
            ("redis", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "dotnet-grpc",
        description: "gRPC service in ASP.NET Core with protobuf tooling",
        category: "Backend",
        tools: &[
            ("dotnet", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("httpie", "latest"),
            ("gh", "latest"),
            ("grpcurl", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "dotnet-mvc",
        description: "ASP.NET Core MVC web app with Razor views and EF Core",
        category: "Backend",
        tools: &[
            ("dotnet", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("httpie", "latest"),
            ("gh", "latest"),
        ],
    },
    // Data & ML
    BuiltinTemplate {
        name: "python-ml",
        description: "Python ML/Data Science with Jupyter and scientific tools",
        category: "Data & ML",
        tools: &[
            ("python", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("gh", "latest"),
            ("awscli", "latest"),
            ("duckdb", "latest"),
            ("bat", "latest"),
            ("fzf", "latest"),
            ("httpie", "latest"),
            ("sqlite", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "jupyter",
        description: "Jupyter notebook environment with essential data tools",
        category: "Data & ML",
        tools: &[
            ("python", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("gh", "latest"),
            ("duckdb", "latest"),
            ("sqlite", "latest"),
        ],
    },
    // DevOps
    BuiltinTemplate {
        name: "k8s-admin",
        description: "Kubernetes administration with cluster management tools",
        category: "DevOps",
        tools: &[
            ("kubectl", "latest"),
            ("helm", "latest"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("k9s", "latest"),
            ("stern", "latest"),
            ("kubectx", "latest"),
            ("gh", "latest"),
            ("awscli", "latest"),
            ("yq", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "terraform",
        description: "Infrastructure as Code with Terraform and cloud CLIs",
        category: "DevOps",
        tools: &[
            ("terraform", "latest"),
            ("git", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("gh", "latest"),
            ("awscli", "latest"),
            ("yq", "latest"),
            ("terragrunt", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "docker-dev",
        description: "Docker development with compose and debugging tools",
        category: "DevOps",
        tools: &[
            ("docker", "latest"),
            ("git", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("dive", "latest"),
            ("lazydocker", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "cicd",
        description: "CI/CD pipeline tools for building and deploying",
        category: "DevOps",
        tools: &[
            ("git", "latest"),
            ("docker", "latest"),
            ("gh", "latest"),
            ("jq", "latest"),
            ("yq", "latest"),
            ("act", "latest"),
            ("awscli", "latest"),
            ("trivy", "latest"),
        ],
    },
    // Mobile
    BuiltinTemplate {
        name: "flutter",
        description: "Flutter cross-platform development with mobile tooling",
        category: "Mobile",
        tools: &[
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("gh", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "react-native",
        description: "React Native mobile development with iOS/Android tooling",
        category: "Mobile",
        tools: &[
            ("node", "20"),
            ("git", "latest"),
            ("docker", "latest"),
            ("jq", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("gh", "latest"),
        ],
    },
    // Minimal
    BuiltinTemplate {
        name: "essential",
        description: "Minimal toolkit with git, editor support, and shell tools",
        category: "Minimal",
        tools: &[
            ("git", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("bat", "latest"),
            ("jq", "latest"),
        ],
    },
    BuiltinTemplate {
        name: "shell-power",
        description: "Power shell user toolkit with productivity enhancers",
        category: "Minimal",
        tools: &[
            ("git", "latest"),
            ("ripgrep", "latest"),
            ("fd", "latest"),
            ("bat", "latest"),
            ("eza", "latest"),
            ("fzf", "latest"),
            ("starship", "latest"),
            ("zoxide", "latest"),
        ],
    },
];

/// Get a built-in template by name
pub fn get_builtin_template(name: &str) -> Option<&'static BuiltinTemplate> {
    BUILTIN_TEMPLATES.iter().find(|t| t.name == name)
}

/// List all built-in templates
pub fn list_builtin_templates() -> &'static [BuiltinTemplate] {
    BUILTIN_TEMPLATES
}

/// Get templates by category
pub fn templates_by_category(category: &str) -> Vec<&'static BuiltinTemplate> {
    BUILTIN_TEMPLATES
        .iter()
        .filter(|t| t.category.eq_ignore_ascii_case(category))
        .collect()
}

/// Get all unique categories
pub fn all_categories() -> Vec<&'static str> {
    let mut categories: Vec<&str> = BUILTIN_TEMPLATES.iter().map(|t| t.category).collect();
    categories.sort();
    categories.dedup();
    categories
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_template() {
        let template = get_builtin_template("react").unwrap();
        assert_eq!(template.name, "react");
        assert!(!template.tools.is_empty());
    }

    #[test]
    fn test_get_nonexistent_template() {
        assert!(get_builtin_template("nonexistent").is_none());
    }

    #[test]
    fn test_list_builtin_templates() {
        let templates = list_builtin_templates();
        assert!(!templates.is_empty());
        assert!(templates.len() >= 20);
    }

    #[test]
    fn test_templates_by_category() {
        let web_templates = templates_by_category("Web Development");
        assert!(!web_templates.is_empty());
        assert!(web_templates.iter().any(|t| t.name == "react"));
    }

    #[test]
    fn test_all_categories() {
        let categories = all_categories();
        assert!(categories.contains(&"Web Development"));
        assert!(categories.contains(&"Backend"));
        assert!(categories.contains(&"DevOps"));
    }

    #[test]
    fn test_builtin_template_to_template() {
        let builtin = get_builtin_template("essential").unwrap();
        let template = builtin.to_template();
        assert_eq!(template.name(), "essential");
        assert_eq!(template.tool_count(), 5);
    }
}
