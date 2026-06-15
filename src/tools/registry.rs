//! Tool registry for mapping tool names to installation handlers.
//!
//! # Thread Safety
//!
//! The registry uses an `RwLock` for thread-safe access. Lock poisoning is treated
//! as a fatal error (panic) because:
//!
//! 1. This is a CLI tool, not a long-running service - recovery is unnecessary
//! 2. A poisoned lock indicates a prior panic during registration, which is a bug
//! 3. Continuing with potentially corrupted state would be unsafe
//!
//! The `expect()` calls on lock acquisition are intentional and document this design.

#![allow(dead_code)] // Public API for tool registration and lookup

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::tools::common::InstallError;

/// Function signature for tool installation handlers.
pub type ToolAdder = fn(version: &str) -> Result<(), InstallError>;

/// Global registry mapping tool name -> handler.
/// Keys are stored in lowercase for case-insensitive lookups.
static REGISTRY: OnceLock<RwLock<HashMap<String, ToolAdder>>> = OnceLock::new();

#[inline]
fn registry() -> &'static RwLock<HashMap<String, ToolAdder>> {
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register a tool handler under the given name.
/// Returns true if a new entry was inserted, false if an existing entry was replaced.
#[must_use = "indicates whether entry was new or replaced"]
pub fn register_tool(name: &str, handler: ToolAdder) -> bool {
    let key = name.to_ascii_lowercase();
    let mut map = registry().write().expect("registry rwlock poisoned");
    map.insert(key, handler).is_none()
}

/// Retrieve a registered tool handler by name, if present.
///
/// Lookup is case-insensitive AND tolerates the user typing either
/// hyphen or underscore — `nats-server` and `nats_server` both
/// resolve to the same tool. This closes the gap from the messaging
/// review (QA F4): `define_tool!(NATS_SERVER, ...)` stringifies as
/// `"nats_server"`, but every upstream doc and command shows
/// `nats-server`. The natural user mistake is to write the hyphen
/// form, which would otherwise produce "Unknown tool" with a fuzzy
/// suggestion.
#[inline]
pub fn get_tool(name: &str) -> Option<ToolAdder> {
    let key = name.to_ascii_lowercase();
    let map = registry().read().expect("registry rwlock poisoned");
    if let Some(handler) = map.get(&key).copied() {
        return Some(handler);
    }
    // Fall back to dash↔underscore aliasing.
    if key.contains('-') {
        let alias = key.replace('-', "_");
        if let Some(handler) = map.get(&alias).copied() {
            return Some(handler);
        }
    }
    if key.contains('_') {
        let alias = key.replace('_', "-");
        if let Some(handler) = map.get(&alias).copied() {
            return Some(handler);
        }
    }
    None
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
///
/// Resolution order:
/// 1. User-defined plugin tools (`~/.jarvy/tools.d/`). Plugins dispatch by
///    their declared `name` so the package fields belonging to that plugin
///    are the ones executed — no shared-handler ambiguity.
/// 2. Built-in tool registry.
///
/// Returns `InstallError::Parse("unknown tool")` if neither has a handler.
#[must_use = "this Result may contain an error that should be handled"]
pub fn add(name: &str, version: &str) -> Result<(), InstallError> {
    // Plugins first: name-keyed dispatch is correct by construction.
    match crate::tools::plugins::install_by_name(name, version) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => return Err(e),
    }

    // Route through `get_tool` so the dash↔underscore aliasing
    // applied to lookups also applies to install dispatch — without
    // this, `jarvy validate` accepts `nats-server` (alias) but
    // `jarvy setup` fails to install it.
    if let Some(handler) = get_tool(name) {
        handler(version)
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
    fn get_tool_resolves_hyphen_when_registered_with_underscore() {
        // The motivating case: define_tool!(NATS_SERVER, ...) stringifies
        // to "nats_server", but every upstream doc shows "nats-server".
        let name = "nats_server_alias_test";
        let _ = register_tool(name, dummy_handler);
        // Underscore form works (canonical).
        assert!(get_tool("nats_server_alias_test").is_some());
        // Hyphen form works (the alias).
        assert!(get_tool("nats-server-alias-test").is_some());
    }

    #[test]
    fn get_tool_resolves_underscore_when_registered_with_hyphen() {
        let name = "some-dashed-tool";
        let _ = register_tool(name, dummy_handler);
        assert!(get_tool("some-dashed-tool").is_some());
        assert!(get_tool("some_dashed_tool").is_some());
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
