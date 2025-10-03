use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::tools::common::InstallError;

// Public function type for tool "add" handlers
pub type ToolAdder = fn(version: &str) -> Result<(), InstallError>;

// Global registry mapping tool name -> handler
// Keys are stored in the lowercase for case-insensitive lookups.
static REGISTRY: OnceLock<RwLock<HashMap<String, ToolAdder>>> = OnceLock::new();

fn registry() -> &'static RwLock<HashMap<String, ToolAdder>> {
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register a tool handler under the given name.
/// Returns true if a new entry was inserted, false if an existing entry was replaced.
pub fn register_tool(name: &str, handler: ToolAdder) -> bool {
    let key = name.to_ascii_lowercase();
    let mut map = registry().write().expect("registry rwlock poisoned");
    map.insert(key, handler).is_none()
}

/// Retrieve a registered tool handler by name, if present.
/// Lookup is case-insensitive.
pub fn get_tool(name: &str) -> Option<ToolAdder> {
    let key = name.to_ascii_lowercase();
    let map = registry().read().expect("registry rwlock poisoned");
    map.get(&key).copied()
}

/// List all registered tool names (lowercased), sorted for determinism.
pub fn registered_tool_names() -> Vec<String> {
    let map = registry().read().expect("registry rwlock poisoned");
    let mut names: Vec<String> = map.keys().cloned().collect();
    names.sort();
    names
}

/// Dispatch an added request to a registered tool by name and version.
/// Example: add("git", "latest") or add("docker", "24.01").
/// Returns InstallError::Parse("unknown tool") if no handler is registered.
pub fn add(name: &str, version: &str) -> Result<(), InstallError> {
    let key = name.to_ascii_lowercase();
    let map = registry().read().expect("registry rwlock poisoned");
    if let Some(handler) = map.get(&key) {
        // clone the function pointer out while holding read lock
        let f = *handler;
        drop(map);
        f(version)
    } else {
        Err(InstallError::Parse("unknown tool"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_handler(_version: &str) -> Result<(), InstallError> {
        Ok(())
    }

    #[test]
    fn get_tool_returns_some_after_register() {
        let name = "TeStToOl_get";
        let _ = register_tool(name, dummy_handler);
        let h = get_tool("testtool_get");
        assert!(h.is_some());
        let f = h.unwrap();
        assert!(f("any").is_ok());
    }

    #[test]
    fn get_tool_returns_none_for_unknown() {
        let h = get_tool("definitely-unknown-tool-name");
        assert!(h.is_none());
    }
}

// Tool struct for registry usage: wraps a name and an add handler
#[derive(Clone)]
pub struct Tool {
    pub name: String,
    handler: ToolAdder,
}

impl Tool {
    pub fn new(name: &str, handler: ToolAdder) -> Self {
        Self {
            name: name.to_string(),
            handler,
        }
    }

    /// Invoke this tool's add/install logic for a given version hint
    pub fn add(&self, version: &str) -> Result<(), InstallError> {
        (self.handler)(version)
    }
}

/// Register a Tool by extracting its name and handler.
/// Returns true if inserted new, false if replaced existing.
pub fn register_tool_struct(tool: &Tool) -> bool {
    register_tool(&tool.name, tool.handler)
}

/// Retrieve a Tool by name if present (case-insensitive).
/// The returned Tool carries the looked-up name in lowercase for consistency.
pub fn get_tool_struct(name: &str) -> Option<Tool> {
    get_tool(name).map(|handler| Tool {
        name: name.to_ascii_lowercase(),
        handler,
    })
}

#[cfg(test)]
mod tests2 {
    use super::*;

    fn dummy_struct_handler(_version: &str) -> Result<(), InstallError> {
        Ok(())
    }

    #[test]
    fn register_and_use_tool_struct() {
        let my = Tool::new("TeStToOl_struct", dummy_struct_handler);
        let _ = register_tool_struct(&my);

        let fetched = get_tool_struct("testtool_struct").expect("tool should be present");
        assert_eq!(fetched.name, "testtool_struct");
        assert!(fetched.add("any").is_ok());
    }
}
