//! Role Resolution with Inheritance
//!
//! Resolves role definitions by following inheritance chains and merging tools.
//! Implements depth-first resolution with cycle detection.

#![allow(dead_code)] // Public API for role resolution

use super::MAX_INHERITANCE_DEPTH;
use super::definition::{RoleDefinitionWrapper, RolesConfig};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Errors that can occur during role resolution
#[derive(Debug, Clone, PartialEq)]
pub enum RoleResolverError {
    /// Role not found in config
    RoleNotFound(String),
    /// Circular inheritance detected
    CircularInheritance { role: String, chain: Vec<String> },
    /// Maximum inheritance depth exceeded
    MaxDepthExceeded { role: String, depth: usize },
    /// Parent role not found during inheritance resolution
    ParentNotFound { child: String, parent: String },
}

impl fmt::Display for RoleResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoleResolverError::RoleNotFound(name) => {
                write!(f, "Role '{}' not found in configuration", name)
            }
            RoleResolverError::CircularInheritance { role, chain } => {
                write!(
                    f,
                    "Circular inheritance detected for role '{}': {}",
                    role,
                    chain.join(" -> ")
                )
            }
            RoleResolverError::MaxDepthExceeded { role, depth } => {
                write!(
                    f,
                    "Maximum inheritance depth ({}) exceeded for role '{}'. Max allowed: {}",
                    depth, role, MAX_INHERITANCE_DEPTH
                )
            }
            RoleResolverError::ParentNotFound { child, parent } => {
                write!(
                    f,
                    "Parent role '{}' not found (referenced by role '{}')",
                    parent, child
                )
            }
        }
    }
}

impl std::error::Error for RoleResolverError {}

/// A fully resolved role with all inherited tools merged
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRole {
    /// Role name
    pub name: String,
    /// Description (from this role or first parent with description)
    pub description: Option<String>,
    /// All tools with versions (merged from inheritance chain)
    pub tools: HashMap<String, ResolvedTool>,
    /// Inheritance chain showing how tools were resolved
    pub inheritance_chain: Vec<String>,
}

/// A resolved tool with version and source tracking
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTool {
    /// Tool version
    pub version: String,
    /// Whether to use version manager
    pub version_manager: bool,
    /// Sudo override
    pub use_sudo: Option<bool>,
    /// Which role this tool came from
    pub source_role: String,
}

impl ResolvedRole {
    /// Get just the tool names
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(String::as_str).collect()
    }

    /// Get tools as name -> version map
    pub fn tools_map(&self) -> HashMap<String, String> {
        self.tools
            .iter()
            .map(|(name, tool)| (name.clone(), tool.version.clone()))
            .collect()
    }

    /// Count of tools
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }
}

/// Role resolver that handles inheritance and caching
pub struct RoleResolver<'a> {
    /// Reference to roles config
    config: &'a RolesConfig,
    /// Cache of already resolved roles
    cache: HashMap<String, ResolvedRole>,
}

impl<'a> RoleResolver<'a> {
    /// Create a new resolver for the given roles config
    pub fn new(config: &'a RolesConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
        }
    }

    /// Resolve a single role by name
    pub fn resolve(&mut self, role_name: &str) -> Result<ResolvedRole, RoleResolverError> {
        // Check cache first
        if let Some(cached) = self.cache.get(role_name) {
            return Ok(cached.clone());
        }

        // Start resolution with empty visited set
        let mut visited = HashSet::new();
        let result = self.resolve_internal(role_name, &mut visited, 0)?;

        // Cache the result
        self.cache.insert(role_name.to_string(), result.clone());

        Ok(result)
    }

    /// Resolve multiple roles and merge their tools
    /// Later roles in the list override earlier ones (last wins)
    pub fn resolve_multiple(
        &mut self,
        role_names: &[&str],
    ) -> Result<ResolvedRole, RoleResolverError> {
        if role_names.is_empty() {
            return Ok(ResolvedRole {
                name: String::new(),
                description: None,
                tools: HashMap::new(),
                inheritance_chain: Vec::new(),
            });
        }

        if role_names.len() == 1 {
            return self.resolve(role_names[0]);
        }

        // Resolve all roles
        let mut resolved_roles = Vec::with_capacity(role_names.len());
        for name in role_names {
            resolved_roles.push(self.resolve(name)?);
        }

        // Merge tools - last role wins for conflicts
        // Use moves instead of clones by taking ownership of resolved_roles
        let mut merged_tools = HashMap::new();
        let mut merged_chain = Vec::with_capacity(resolved_roles.len());

        // First pass: collect description from last role that has one (before consuming)
        let description = resolved_roles
            .iter()
            .rev()
            .find_map(|r| r.description.as_ref())
            .cloned();

        // Second pass: consume resolved_roles, moving data instead of cloning
        for role in resolved_roles {
            merged_chain.push(role.name); // Move instead of clone
            for (tool_name, tool) in role.tools {
                // Move both key and value
                merged_tools.insert(tool_name, tool);
            }
        }

        Ok(ResolvedRole {
            name: role_names.join("+"),
            description,
            tools: merged_tools,
            inheritance_chain: merged_chain,
        })
    }

    /// Internal recursive resolution with cycle detection
    fn resolve_internal(
        &self,
        role_name: &str,
        visited: &mut HashSet<String>,
        depth: usize,
    ) -> Result<ResolvedRole, RoleResolverError> {
        // Check depth limit
        if depth > MAX_INHERITANCE_DEPTH {
            return Err(RoleResolverError::MaxDepthExceeded {
                role: role_name.to_string(),
                depth,
            });
        }

        // Check for circular reference
        if visited.contains(role_name) {
            return Err(RoleResolverError::CircularInheritance {
                role: role_name.to_string(),
                chain: visited.iter().cloned().collect(),
            });
        }

        // Find the role definition
        let role_def = self
            .config
            .roles
            .get(role_name)
            .ok_or_else(|| RoleResolverError::RoleNotFound(role_name.to_string()))?;

        // Mark as visited
        visited.insert(role_name.to_string());

        // Start with empty tools map
        let mut tools = HashMap::new();
        let mut inheritance_chain = Vec::new();

        // If this role extends others, resolve parents first
        if role_def.has_extends() {
            let parent_names = role_def.get_extends();
            for parent_name in parent_names {
                // Check parent exists
                if !self.config.roles.contains_key(parent_name) {
                    return Err(RoleResolverError::ParentNotFound {
                        child: role_name.to_string(),
                        parent: parent_name.to_string(),
                    });
                }

                // Resolve parent
                let parent_resolved = self.resolve_internal(parent_name, visited, depth + 1)?;

                // Add parent's inheritance chain
                inheritance_chain.extend(parent_resolved.inheritance_chain);

                // Merge parent tools (earlier parents are overridden by later ones)
                for (tool_name, tool) in parent_resolved.tools {
                    tools.insert(tool_name, tool);
                }
            }
        }

        // Add this role to the chain
        inheritance_chain.push(role_name.to_string());

        // Convert wrapper to definition for tool access
        let definition = role_def.clone().into_definition();

        // Add/override with this role's tools
        for tool_name in &definition.tools {
            // Simple tool from array - get version from tool_versions if available
            let version = definition
                .tool_versions
                .get(tool_name)
                .map(|spec| spec.version().to_string())
                .unwrap_or_else(|| "latest".to_string());

            let (version_manager, use_sudo) = definition
                .tool_versions
                .get(tool_name)
                .map(|spec| (spec.version_manager(), spec.use_sudo()))
                .unwrap_or((true, None));

            tools.insert(
                tool_name.clone(),
                ResolvedTool {
                    version,
                    version_manager,
                    use_sudo,
                    source_role: role_name.to_string(),
                },
            );
        }

        // Add tools from tool_versions that aren't in the simple tools list
        for (tool_name, spec) in &definition.tool_versions {
            if tool_name != "tools" && !definition.tools.contains(tool_name) {
                tools.insert(
                    tool_name.clone(),
                    ResolvedTool {
                        version: spec.version().to_string(),
                        version_manager: spec.version_manager(),
                        use_sudo: spec.use_sudo(),
                        source_role: role_name.to_string(),
                    },
                );
            }
        }

        // Remove from visited (allow other branches to visit)
        visited.remove(role_name);

        Ok(ResolvedRole {
            name: role_name.to_string(),
            description: definition.description,
            tools,
            inheritance_chain,
        })
    }

    /// Get all role names from the config
    pub fn list_roles(&self) -> Vec<&str> {
        self.config.roles.keys().map(String::as_str).collect()
    }

    /// Check if a role exists
    pub fn role_exists(&self, name: &str) -> bool {
        self.config.roles.contains_key(name)
    }

    /// Get role definition (unresolved)
    pub fn get_role(&self, name: &str) -> Option<&RoleDefinitionWrapper> {
        self.config.roles.get(name)
    }

    /// Clear the resolution cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

/// Compute the difference between two resolved roles
pub fn diff_roles(role_a: &ResolvedRole, role_b: &ResolvedRole) -> RoleDiff {
    let mut only_in_a = HashMap::new();
    let mut only_in_b = HashMap::new();
    let mut different_versions = HashMap::new();
    let mut same = HashMap::new();

    // Check all tools in A
    for (name, tool_a) in &role_a.tools {
        if let Some(tool_b) = role_b.tools.get(name) {
            if tool_a.version == tool_b.version {
                same.insert(name.clone(), tool_a.version.clone());
            } else {
                different_versions.insert(
                    name.clone(),
                    (tool_a.version.clone(), tool_b.version.clone()),
                );
            }
        } else {
            only_in_a.insert(name.clone(), tool_a.version.clone());
        }
    }

    // Check tools only in B
    for (name, tool_b) in &role_b.tools {
        if !role_a.tools.contains_key(name) {
            only_in_b.insert(name.clone(), tool_b.version.clone());
        }
    }

    RoleDiff {
        role_a: role_a.name.clone(),
        role_b: role_b.name.clone(),
        only_in_a,
        only_in_b,
        different_versions,
        same,
    }
}

/// Difference between two roles
#[derive(Debug, Clone)]
pub struct RoleDiff {
    pub role_a: String,
    pub role_b: String,
    /// Tools only in role A (name -> version)
    pub only_in_a: HashMap<String, String>,
    /// Tools only in role B (name -> version)
    pub only_in_b: HashMap<String, String>,
    /// Tools with different versions (name -> (version_a, version_b))
    pub different_versions: HashMap<String, (String, String)>,
    /// Tools that are the same (name -> version)
    pub same: HashMap<String, String>,
}

impl RoleDiff {
    /// Check if roles are identical
    pub fn is_identical(&self) -> bool {
        self.only_in_a.is_empty() && self.only_in_b.is_empty() && self.different_versions.is_empty()
    }

    /// Get total number of differences
    pub fn difference_count(&self) -> usize {
        self.only_in_a.len() + self.only_in_b.len() + self.different_versions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::definition::{RoleDefinition, RoleExtends, RoleToolSpec};

    fn create_test_config() -> RolesConfig {
        let mut roles = HashMap::new();

        // Base role
        let base_def = RoleDefinition {
            description: Some("Base development tools".to_string()),
            tools: vec!["git".to_string(), "docker".to_string()],
            ..Default::default()
        };
        roles.insert("base".to_string(), RoleDefinitionWrapper::Simple(base_def));

        // Frontend role that extends base
        let mut frontend_def = RoleDefinition {
            description: Some("Frontend development".to_string()),
            extends: Some(RoleExtends::Single("base".to_string())),
            tools: vec!["node".to_string(), "bun".to_string()],
            ..Default::default()
        };
        frontend_def
            .tool_versions
            .insert("node".to_string(), RoleToolSpec::Simple("20".to_string()));
        roles.insert(
            "frontend".to_string(),
            RoleDefinitionWrapper::Simple(frontend_def),
        );

        // Backend role that extends base
        let backend_def = RoleDefinition {
            extends: Some(RoleExtends::Single("base".to_string())),
            tools: vec!["rust".to_string(), "go".to_string()],
            ..Default::default()
        };
        roles.insert(
            "backend".to_string(),
            RoleDefinitionWrapper::Simple(backend_def),
        );

        // Senior frontend that extends frontend
        let senior_def = RoleDefinition {
            extends: Some(RoleExtends::Single("frontend".to_string())),
            tools: vec!["kubectl".to_string()],
            ..Default::default()
        };
        roles.insert(
            "senior-frontend".to_string(),
            RoleDefinitionWrapper::Simple(senior_def),
        );

        // Fullstack that extends both frontend and backend
        let fullstack_def = RoleDefinition {
            extends: Some(RoleExtends::Multiple(vec![
                "frontend".to_string(),
                "backend".to_string(),
            ])),
            ..Default::default()
        };
        roles.insert(
            "fullstack".to_string(),
            RoleDefinitionWrapper::Simple(fullstack_def),
        );

        RolesConfig { roles }
    }

    #[test]
    fn test_resolve_simple_role() {
        let config = create_test_config();
        let mut resolver = RoleResolver::new(&config);

        let resolved = resolver.resolve("base").unwrap();
        assert_eq!(resolved.name, "base");
        assert_eq!(
            resolved.description,
            Some("Base development tools".to_string())
        );
        assert_eq!(resolved.tools.len(), 2);
        assert!(resolved.tools.contains_key("git"));
        assert!(resolved.tools.contains_key("docker"));
        assert_eq!(resolved.inheritance_chain, vec!["base"]);
    }

    #[test]
    fn test_resolve_with_inheritance() {
        let config = create_test_config();
        let mut resolver = RoleResolver::new(&config);

        let resolved = resolver.resolve("frontend").unwrap();
        assert_eq!(resolved.name, "frontend");
        // Should have base tools + frontend tools
        assert!(resolved.tools.contains_key("git")); // from base
        assert!(resolved.tools.contains_key("docker")); // from base
        assert!(resolved.tools.contains_key("node")); // from frontend
        assert!(resolved.tools.contains_key("bun")); // from frontend
        assert_eq!(resolved.tools.get("node").unwrap().version, "20");
        assert_eq!(resolved.inheritance_chain, vec!["base", "frontend"]);
    }

    #[test]
    fn test_resolve_deep_inheritance() {
        let config = create_test_config();
        let mut resolver = RoleResolver::new(&config);

        let resolved = resolver.resolve("senior-frontend").unwrap();
        // Should have base + frontend + senior-frontend tools
        assert!(resolved.tools.contains_key("git")); // from base
        assert!(resolved.tools.contains_key("node")); // from frontend
        assert!(resolved.tools.contains_key("kubectl")); // from senior-frontend
        assert_eq!(
            resolved.inheritance_chain,
            vec!["base", "frontend", "senior-frontend"]
        );
    }

    #[test]
    fn test_resolve_multiple_inheritance() {
        let config = create_test_config();
        let mut resolver = RoleResolver::new(&config);

        let resolved = resolver.resolve("fullstack").unwrap();
        // Should have frontend tools + backend tools
        assert!(resolved.tools.contains_key("node")); // from frontend
        assert!(resolved.tools.contains_key("rust")); // from backend
        assert!(resolved.tools.contains_key("git")); // from base (via both)
    }

    #[test]
    fn test_resolve_nonexistent_role() {
        let config = create_test_config();
        let mut resolver = RoleResolver::new(&config);

        let result = resolver.resolve("nonexistent");
        assert!(matches!(result, Err(RoleResolverError::RoleNotFound(_))));
    }

    #[test]
    fn test_circular_inheritance_detection() {
        let mut config = RolesConfig::default();

        // Create circular: A -> B -> C -> A
        let role_a = RoleDefinition {
            extends: Some(RoleExtends::Single("role_c".to_string())),
            ..Default::default()
        };
        config
            .roles
            .insert("role_a".to_string(), RoleDefinitionWrapper::Simple(role_a));

        let role_b = RoleDefinition {
            extends: Some(RoleExtends::Single("role_a".to_string())),
            ..Default::default()
        };
        config
            .roles
            .insert("role_b".to_string(), RoleDefinitionWrapper::Simple(role_b));

        let role_c = RoleDefinition {
            extends: Some(RoleExtends::Single("role_b".to_string())),
            ..Default::default()
        };
        config
            .roles
            .insert("role_c".to_string(), RoleDefinitionWrapper::Simple(role_c));

        let mut resolver = RoleResolver::new(&config);
        let result = resolver.resolve("role_a");

        assert!(matches!(
            result,
            Err(RoleResolverError::CircularInheritance { .. })
        ));
    }

    /// Build a synthetic linear inheritance chain `r0 <- r1 <- ... <- rN`
    /// for the depth-limit tests below.
    fn linear_chain_config(length: usize) -> RolesConfig {
        let mut config = RolesConfig::default();
        for i in 0..length {
            let extends = if i == 0 {
                None
            } else {
                Some(RoleExtends::Single(format!("r{}", i - 1)))
            };
            let def = RoleDefinition {
                extends,
                ..Default::default()
            };
            config
                .roles
                .insert(format!("r{i}"), RoleDefinitionWrapper::Simple(def));
        }
        config
    }

    #[test]
    fn role_inheritance_at_max_depth_succeeds() {
        // r0 <- r1 <- r2 <- r3 <- r4 <- r5 — exactly MAX_INHERITANCE_DEPTH.
        let config = linear_chain_config(MAX_INHERITANCE_DEPTH + 1);
        let mut resolver = RoleResolver::new(&config);
        let resolved = resolver
            .resolve(&format!("r{}", MAX_INHERITANCE_DEPTH))
            .expect("max-depth chain should resolve");
        assert_eq!(resolved.inheritance_chain.len(), MAX_INHERITANCE_DEPTH + 1);
    }

    #[test]
    fn role_inheritance_above_max_depth_returns_error() {
        // r0 <- r1 <- r2 <- r3 <- r4 <- r5 <- r6 — one over the limit.
        let config = linear_chain_config(MAX_INHERITANCE_DEPTH + 2);
        let mut resolver = RoleResolver::new(&config);
        let result = resolver.resolve(&format!("r{}", MAX_INHERITANCE_DEPTH + 1));
        assert!(
            matches!(result, Err(RoleResolverError::MaxDepthExceeded { .. })),
            "expected MaxDepthExceeded, got {result:?}"
        );
    }

    #[test]
    fn test_resolve_multiple_roles() {
        let config = create_test_config();
        let mut resolver = RoleResolver::new(&config);

        let resolved = resolver.resolve_multiple(&["frontend", "backend"]).unwrap();
        assert!(resolved.tools.contains_key("node")); // from frontend
        assert!(resolved.tools.contains_key("rust")); // from backend
        assert!(resolved.tools.contains_key("git")); // from base (via both)
    }

    #[test]
    fn test_diff_roles() {
        let config = create_test_config();
        let mut resolver = RoleResolver::new(&config);

        let frontend = resolver.resolve("frontend").unwrap();
        let backend = resolver.resolve("backend").unwrap();

        let diff = diff_roles(&frontend, &backend);

        assert!(!diff.is_identical());
        // node and bun only in frontend
        assert!(diff.only_in_a.contains_key("node"));
        assert!(diff.only_in_a.contains_key("bun"));
        // rust and go only in backend
        assert!(diff.only_in_b.contains_key("rust"));
        assert!(diff.only_in_b.contains_key("go"));
        // git and docker are the same (from base)
        assert!(diff.same.contains_key("git"));
        assert!(diff.same.contains_key("docker"));
    }

    #[test]
    fn test_list_roles() {
        let config = create_test_config();
        let resolver = RoleResolver::new(&config);

        let roles = resolver.list_roles();
        assert!(roles.contains(&"base"));
        assert!(roles.contains(&"frontend"));
        assert!(roles.contains(&"backend"));
    }

    #[test]
    fn test_caching() {
        let config = create_test_config();
        let mut resolver = RoleResolver::new(&config);

        // First resolution
        let resolved1 = resolver.resolve("frontend").unwrap();
        // Second resolution should use cache
        let resolved2 = resolver.resolve("frontend").unwrap();

        assert_eq!(resolved1, resolved2);
    }
}
