//! Role Definition Types
//!
//! Defines the structure of roles in jarvy.toml:
//! ```toml
//! [roles.frontend]
//! description = "Frontend development stack"
//! tools = ["node", "bun", "pnpm"]
//!
//! [roles.frontend.tools]
//! node = "20"

#![allow(dead_code)] // Public API for role definitions
//! bun = "latest"
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Role assignment - can be single or multiple roles
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RoleAssignment {
    /// Single role assignment: `role = "frontend"`
    Single(String),
    /// Multiple role assignment: `roles = ["frontend", "devops"]`
    Multiple(Vec<String>),
}

impl RoleAssignment {
    /// Get all assigned roles as a vector
    pub fn as_vec(&self) -> Vec<&str> {
        match self {
            RoleAssignment::Single(s) => vec![s.as_str()],
            RoleAssignment::Multiple(v) => v.iter().map(String::as_str).collect(),
        }
    }

    /// Check if any roles are assigned
    pub fn is_empty(&self) -> bool {
        match self {
            RoleAssignment::Single(s) => s.is_empty(),
            RoleAssignment::Multiple(v) => v.is_empty(),
        }
    }

    /// Get the number of assigned roles
    pub fn len(&self) -> usize {
        match self {
            RoleAssignment::Single(s) => {
                if s.is_empty() {
                    0
                } else {
                    1
                }
            }
            RoleAssignment::Multiple(v) => v.len(),
        }
    }
}

impl Default for RoleAssignment {
    fn default() -> Self {
        RoleAssignment::Multiple(Vec::new())
    }
}

/// Role inheritance - can extend single or multiple parent roles
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RoleExtends {
    /// Single parent: `extends = "base-role"`
    Single(String),
    /// Multiple parents: `extends = ["role-a", "role-b"]`
    Multiple(Vec<String>),
}

impl RoleExtends {
    /// Get all parent role names as a vector
    pub fn as_vec(&self) -> Vec<&str> {
        match self {
            RoleExtends::Single(s) => vec![s.as_str()],
            RoleExtends::Multiple(v) => v.iter().map(String::as_str).collect(),
        }
    }

    /// Check if extends is empty
    pub fn is_empty(&self) -> bool {
        match self {
            RoleExtends::Single(s) => s.is_empty(),
            RoleExtends::Multiple(v) => v.is_empty(),
        }
    }
}

impl Default for RoleExtends {
    fn default() -> Self {
        RoleExtends::Multiple(Vec::new())
    }
}

/// Tool specification in a role - can be simple name or detailed config
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RoleToolSpec {
    /// Detailed tool config with version and options
    Detailed {
        version: String,
        #[serde(default)]
        version_manager: Option<bool>,
        #[serde(default)]
        use_sudo: Option<bool>,
        #[serde(default)]
        distribution: Option<String>,
        #[serde(default)]
        fallback: Option<bool>,
    },
    /// Simple version string: `"latest"` or `"20"`
    Simple(String),
}

impl RoleToolSpec {
    /// Get the version string
    pub fn version(&self) -> &str {
        match self {
            RoleToolSpec::Detailed { version, .. } => version,
            RoleToolSpec::Simple(v) => v,
        }
    }

    /// Check if version_manager is enabled (defaults to true)
    pub fn version_manager(&self) -> bool {
        match self {
            RoleToolSpec::Detailed {
                version_manager, ..
            } => version_manager.unwrap_or(true),
            RoleToolSpec::Simple(_) => true,
        }
    }

    /// Get sudo override if specified
    pub fn use_sudo(&self) -> Option<bool> {
        match self {
            RoleToolSpec::Detailed { use_sudo, .. } => *use_sudo,
            RoleToolSpec::Simple(_) => None,
        }
    }

    pub fn distribution(&self) -> Option<&str> {
        match self {
            RoleToolSpec::Detailed { distribution, .. } => distribution.as_deref(),
            RoleToolSpec::Simple(_) => None,
        }
    }

    pub fn fallback(&self) -> bool {
        match self {
            RoleToolSpec::Detailed { fallback, .. } => fallback.unwrap_or(true),
            RoleToolSpec::Simple(_) => true,
        }
    }
}

/// A role definition within the config
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct RoleDefinition {
    /// Human-readable description of the role
    #[serde(default)]
    pub description: Option<String>,

    /// Parent role(s) to inherit from
    #[serde(default)]
    pub extends: Option<RoleExtends>,

    /// Simple tool list (tool names with default "latest" version)
    #[serde(default)]
    pub tools: Vec<String>,

    /// Detailed tool specifications with versions
    /// This is populated from [roles.name.tools] section
    #[serde(flatten)]
    pub tool_versions: HashMap<String, RoleToolSpec>,
}

impl RoleDefinition {
    /// Check if this role extends other roles
    pub fn has_extends(&self) -> bool {
        self.extends
            .as_ref()
            .map(|e| !e.is_empty())
            .unwrap_or(false)
    }

    /// Get the list of parent role names
    pub fn get_extends(&self) -> Vec<&str> {
        self.extends
            .as_ref()
            .map(|e| e.as_vec())
            .unwrap_or_default()
    }

    /// Get all tools defined in this role (both simple and detailed)
    /// Returns a map of tool name -> version
    pub fn get_tools(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();

        // Add simple tool list with "latest" version
        for tool in &self.tools {
            result.insert(tool.clone(), "latest".to_string());
        }

        // Add/override with detailed tool specs
        for (name, spec) in &self.tool_versions {
            // Skip the "tools" key which contains the nested table
            if name != "tools" {
                result.insert(name.clone(), spec.version().to_string());
            }
        }

        result
    }

    /// Get the number of tools in this role (not counting inheritance)
    pub fn tool_count(&self) -> usize {
        let mut count = self.tools.len();
        for name in self.tool_versions.keys() {
            if name != "tools" && !self.tools.contains(name) {
                count += 1;
            }
        }
        count
    }
}

/// Roles section in jarvy.toml config
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RolesConfig {
    /// Map of role name -> role definition
    #[serde(flatten)]
    pub roles: HashMap<String, RoleDefinitionWrapper>,
}

/// Wrapper to handle the [roles.name] and [roles.name.tools] TOML structure
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RoleDefinitionWrapper {
    /// Role with tools section: [roles.name] + [roles.name.tools]
    WithTools {
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        extends: Option<RoleExtends>,
        #[serde(default)]
        tools: RoleToolsSection,
    },
    /// Simple role with just tool names array
    Simple(RoleDefinition),
}

/// Tools section can be array of names or table with versions
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RoleToolsSection {
    /// Array of tool names: tools = ["node", "bun"]
    Names(Vec<String>),
    /// Table with versions: [roles.name.tools] node = "20"
    Versions(HashMap<String, RoleToolSpec>),
}

impl Default for RoleToolsSection {
    fn default() -> Self {
        RoleToolsSection::Names(Vec::new())
    }
}

impl RoleDefinitionWrapper {
    /// Convert to RoleDefinition
    pub fn into_definition(self) -> RoleDefinition {
        match self {
            RoleDefinitionWrapper::WithTools {
                description,
                extends,
                tools,
            } => {
                let (tool_list, tool_versions) = match tools {
                    RoleToolsSection::Names(names) => (names, HashMap::new()),
                    RoleToolsSection::Versions(versions) => (Vec::new(), versions),
                };
                RoleDefinition {
                    description,
                    extends,
                    tools: tool_list,
                    tool_versions,
                }
            }
            RoleDefinitionWrapper::Simple(def) => def,
        }
    }

    /// Get reference to RoleDefinition-like data
    pub fn description(&self) -> Option<&str> {
        match self {
            RoleDefinitionWrapper::WithTools { description, .. } => description.as_deref(),
            RoleDefinitionWrapper::Simple(def) => def.description.as_deref(),
        }
    }

    /// Check if extends is specified
    pub fn has_extends(&self) -> bool {
        match self {
            RoleDefinitionWrapper::WithTools { extends, .. } => {
                extends.as_ref().map(|e| !e.is_empty()).unwrap_or(false)
            }
            RoleDefinitionWrapper::Simple(def) => def.has_extends(),
        }
    }

    /// Get extends references
    pub fn get_extends(&self) -> Vec<&str> {
        match self {
            RoleDefinitionWrapper::WithTools { extends, .. } => {
                extends.as_ref().map(|e| e.as_vec()).unwrap_or_default()
            }
            RoleDefinitionWrapper::Simple(def) => def.get_extends(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_assignment_single() {
        let assignment = RoleAssignment::Single("frontend".to_string());
        assert_eq!(assignment.as_vec(), vec!["frontend"]);
        assert_eq!(assignment.len(), 1);
        assert!(!assignment.is_empty());
    }

    #[test]
    fn test_role_assignment_multiple() {
        let assignment =
            RoleAssignment::Multiple(vec!["frontend".to_string(), "devops".to_string()]);
        assert_eq!(assignment.as_vec(), vec!["frontend", "devops"]);
        assert_eq!(assignment.len(), 2);
        assert!(!assignment.is_empty());
    }

    #[test]
    fn test_role_assignment_empty() {
        let assignment = RoleAssignment::Multiple(Vec::new());
        assert!(assignment.is_empty());
        assert_eq!(assignment.len(), 0);

        let single_empty = RoleAssignment::Single(String::new());
        assert!(single_empty.is_empty());
        assert_eq!(single_empty.len(), 0);
    }

    #[test]
    fn test_role_extends_single() {
        let extends = RoleExtends::Single("base".to_string());
        assert_eq!(extends.as_vec(), vec!["base"]);
        assert!(!extends.is_empty());
    }

    #[test]
    fn test_role_extends_multiple() {
        let extends = RoleExtends::Multiple(vec!["base".to_string(), "common".to_string()]);
        assert_eq!(extends.as_vec(), vec!["base", "common"]);
        assert!(!extends.is_empty());
    }

    #[test]
    fn test_role_tool_spec_simple() {
        let spec = RoleToolSpec::Simple("20".to_string());
        assert_eq!(spec.version(), "20");
        assert!(spec.version_manager());
        assert!(spec.use_sudo().is_none());
    }

    #[test]
    fn test_role_tool_spec_detailed() {
        let spec = RoleToolSpec::Detailed {
            version: "18".to_string(),
            version_manager: Some(false),
            use_sudo: Some(true),
            distribution: None,
            fallback: None,
        };
        assert_eq!(spec.version(), "18");
        assert!(!spec.version_manager());
        assert_eq!(spec.use_sudo(), Some(true));
        assert_eq!(spec.distribution(), None);
        assert!(spec.fallback());
    }

    #[test]
    fn role_tool_spec_carries_distribution_and_fallback() {
        let spec = RoleToolSpec::Detailed {
            version: "17".to_string(),
            version_manager: None,
            use_sudo: None,
            distribution: Some("temurin".to_string()),
            fallback: Some(false),
        };
        assert_eq!(spec.distribution(), Some("temurin"));
        assert!(!spec.fallback());
    }

    #[test]
    fn role_tool_spec_detailed_parses_distribution_from_toml() {
        let toml_src = r#"version = "17"
distribution = "temurin"
fallback = false"#;
        let spec: RoleToolSpec = toml::from_str(toml_src).expect("parse RoleToolSpec");
        assert_eq!(spec.version(), "17");
        assert_eq!(spec.distribution(), Some("temurin"));
        assert!(!spec.fallback());
    }

    #[test]
    fn test_role_definition_get_tools() {
        let mut def = RoleDefinition {
            tools: vec!["node".to_string(), "bun".to_string()],
            ..Default::default()
        };
        def.tool_versions
            .insert("node".to_string(), RoleToolSpec::Simple("20".to_string()));
        def.tool_versions
            .insert("pnpm".to_string(), RoleToolSpec::Simple("8".to_string()));

        let tools = def.get_tools();
        assert_eq!(tools.get("node"), Some(&"20".to_string())); // Version overrides "latest"
        assert_eq!(tools.get("bun"), Some(&"latest".to_string()));
        assert_eq!(tools.get("pnpm"), Some(&"8".to_string()));
    }

    #[test]
    fn test_role_definition_has_extends() {
        let mut def = RoleDefinition::default();
        assert!(!def.has_extends());

        def.extends = Some(RoleExtends::Single("base".to_string()));
        assert!(def.has_extends());
    }

    #[test]
    fn test_role_assignment_deserialize_single() {
        let toml_str = r#"role = "frontend""#;
        #[derive(Deserialize)]
        struct Test {
            role: RoleAssignment,
        }
        let test: Test = toml::from_str(toml_str).unwrap();
        assert!(matches!(test.role, RoleAssignment::Single(_)));
    }

    #[test]
    fn test_role_assignment_deserialize_multiple() {
        let toml_str = r#"role = ["frontend", "devops"]"#;
        #[derive(Deserialize)]
        struct Test {
            role: RoleAssignment,
        }
        let test: Test = toml::from_str(toml_str).unwrap();
        assert!(matches!(test.role, RoleAssignment::Multiple(_)));
    }
}
