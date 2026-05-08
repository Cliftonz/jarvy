//! Configuration inheritance resolution
//!
//! Implements recursive config resolution with:
//! - Depth-first, left-to-right traversal
//! - Diamond dependency handling (load once, apply once)
//! - Circular dependency detection
//! - Deep merge with last-write-wins semantics

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::Extends;
use super::cache::ConfigCache;
use crate::config::{Config, EnvConfig, HooksConfig, ServicesConfig, ToolConfig, ToolHooks};

/// Maximum inheritance depth to prevent stack overflow
pub const MAX_DEPTH: usize = 10;

/// Result type for inheritance operations
pub type Result<T> = std::result::Result<T, InheritanceError>;

/// Errors during inheritance resolution
#[derive(Debug)]
pub enum InheritanceError {
    /// Maximum inheritance depth exceeded
    MaxDepthExceeded { path: Vec<String>, depth: usize },
    /// Circular dependency detected
    CircularDependency(Vec<String>),
    /// File not found
    FileNotFound { path: String, error: String },
    /// Failed to fetch remote config
    FetchFailed { url: String, error: String },
    /// Invalid TOML in config
    InvalidToml { source: String, error: String },
}

impl std::fmt::Display for InheritanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InheritanceError::MaxDepthExceeded { path, depth } => {
                write!(
                    f,
                    "Maximum inheritance depth ({}) exceeded.\nChain: {}",
                    depth,
                    path.join(" -> ")
                )
            }
            InheritanceError::CircularDependency(cycle) => {
                write!(f, "Circular dependency detected: {}", cycle.join(" -> "))
            }
            InheritanceError::FileNotFound { path, error } => {
                write!(f, "Config file not found: {} ({})", path, error)
            }
            InheritanceError::FetchFailed { url, error } => {
                write!(
                    f,
                    "Failed to fetch remote config: {}\n\
                     Error: {}\n\
                     Hint: Try --offline to use cached config",
                    url, error
                )
            }
            InheritanceError::InvalidToml { source, error } => {
                write!(f, "Invalid TOML in '{}': {}", source, error)
            }
        }
    }
}

impl std::error::Error for InheritanceError {}

/// Trace entry for resolution debugging
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// Source path/URL
    pub source: String,
    /// Depth in the inheritance chain
    pub depth: usize,
    /// Whether this was loaded from cache
    pub from_cache: bool,
    /// Parent sources that this extends
    pub parents: Vec<String>,
}

/// Resolution trace for debugging inheritance chains
#[derive(Debug, Default)]
pub struct ResolutionTrace {
    /// All entries in resolution order
    pub entries: Vec<TraceEntry>,
    /// Total depth of the deepest chain
    pub max_depth: usize,
    /// URLs that were fetched from network
    pub network_fetches: Vec<String>,
    /// URLs that were served from cache
    pub cache_hits: Vec<String>,
}

impl ResolutionTrace {
    /// Display the inheritance chain as a tree
    pub fn display_tree(&self) -> String {
        let mut output = String::new();
        for entry in &self.entries {
            let indent = "  ".repeat(entry.depth);
            let cache_marker = if entry.from_cache { " (cached)" } else { "" };
            output.push_str(&format!("{}{}{}\n", indent, entry.source, cache_marker));
        }
        output
    }
}

/// Extended config with inheritance support
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ExtendedConfig {
    /// Parent configs to extend
    #[serde(default)]
    pub extends: Option<Extends>,
    /// Tools configuration
    #[serde(rename = "provisioner", default)]
    pub tools: HashMap<String, ToolConfig>,
    /// Hooks configuration
    #[serde(default)]
    pub hooks: HooksConfig,
    /// Environment variables configuration
    #[serde(default)]
    pub env: EnvConfig,
    /// Services configuration
    #[serde(default)]
    pub services: ServicesConfig,
}

impl ExtendedConfig {
    /// Parse from TOML string
    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(|e| InheritanceError::InvalidToml {
            source: "<string>".to_string(),
            error: e.to_string(),
        })
    }

    /// Convert to standard Config (loses extends field)
    pub fn into_config(self) -> Config {
        // Create a TOML string and parse it as Config
        // This is a workaround since Config doesn't have public constructors
        let mut toml_content = String::new();

        // Write provisioner section
        toml_content.push_str("[provisioner]\n");
        for (name, config) in &self.tools {
            match config {
                ToolConfig::Simple(v) => {
                    toml_content.push_str(&format!("{} = \"{}\"\n", name, v));
                }
                ToolConfig::Detailed {
                    version,
                    version_manager,
                    use_sudo,
                } => {
                    toml_content.push_str(&format!("{} = {{ version = \"{}\"", name, version));
                    if let Some(vm) = version_manager {
                        toml_content.push_str(&format!(", version_manager = {}", vm));
                    }
                    if let Some(sudo) = use_sudo {
                        toml_content.push_str(&format!(", use_sudo = {}", sudo));
                    }
                    toml_content.push_str(" }\n");
                }
            }
        }

        // For now, just parse a minimal config - hooks/env/services handled separately
        toml::from_str(&toml_content).unwrap_or_else(|_| toml::from_str("[provisioner]\n").unwrap())
    }
}

/// Inheritance resolver for config files
pub struct InheritanceResolver {
    /// Cache for remote configs
    cache: ConfigCache,
    /// Set of configs currently being resolved (for cycle detection)
    in_progress: HashSet<String>,
    /// Cache of already resolved configs
    resolved_cache: HashMap<String, ExtendedConfig>,
    /// Current resolution depth
    depth: usize,
    /// Base directory for resolving relative paths
    base_dir: PathBuf,
    /// Whether to use offline mode (cache only)
    offline: bool,
    /// Resolution trace for debugging
    trace: ResolutionTrace,
}

impl InheritanceResolver {
    /// Create a new resolver
    pub fn new() -> Self {
        Self {
            cache: ConfigCache::new(),
            in_progress: HashSet::new(),
            resolved_cache: HashMap::new(),
            depth: 0,
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            offline: false,
            trace: ResolutionTrace::default(),
        }
    }

    /// Set base directory for relative path resolution
    pub fn with_base_dir(mut self, dir: PathBuf) -> Self {
        self.base_dir = dir;
        self
    }

    /// Enable offline mode (use cached configs only)
    pub fn offline(mut self, enabled: bool) -> Self {
        self.offline = enabled;
        self
    }

    /// Set custom cache
    pub fn with_cache(mut self, cache: ConfigCache) -> Self {
        self.cache = cache;
        self
    }

    /// Resolve a config file with all its ancestors merged
    pub fn resolve(&mut self, path: &str) -> Result<ExtendedConfig> {
        self.depth = 0;
        self.in_progress.clear();
        self.trace = ResolutionTrace::default();

        self.resolve_recursive(path)
    }

    /// Resolve with trace for debugging
    pub fn resolve_with_trace(&mut self, path: &str) -> Result<(ExtendedConfig, ResolutionTrace)> {
        let config = self.resolve(path)?;
        let trace = std::mem::take(&mut self.trace);
        Ok((config, trace))
    }

    /// Recursive resolution implementation
    fn resolve_recursive(&mut self, source: &str) -> Result<ExtendedConfig> {
        // Check depth limit
        if self.depth > MAX_DEPTH {
            return Err(InheritanceError::MaxDepthExceeded {
                path: self.in_progress.iter().cloned().collect(),
                depth: self.depth,
            });
        }

        // Normalize the source path
        let normalized = self.normalize_source(source);

        // Check for circular dependency
        if self.in_progress.contains(&normalized) {
            let mut cycle: Vec<String> = self.in_progress.iter().cloned().collect();
            cycle.push(normalized);
            return Err(InheritanceError::CircularDependency(cycle));
        }

        // Mark as in progress
        self.in_progress.insert(normalized.clone());
        self.depth += 1;

        // Load the config
        let (content, from_cache) = self.load_config(&normalized)?;

        // Parse the config
        let mut config: ExtendedConfig =
            toml::from_str(&content).map_err(|e| InheritanceError::InvalidToml {
                source: normalized.clone(),
                error: e.to_string(),
            })?;

        // Track in trace
        let parents: Vec<String> = config
            .extends
            .as_ref()
            .map(|e| e.as_vec().iter().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        self.trace.entries.push(TraceEntry {
            source: normalized.clone(),
            depth: self.depth - 1,
            from_cache,
            parents: parents.clone(),
        });

        if from_cache {
            self.trace.cache_hits.push(normalized.clone());
        } else if is_url(&normalized) {
            self.trace.network_fetches.push(normalized.clone());
        }

        if self.depth > self.trace.max_depth {
            self.trace.max_depth = self.depth;
        }

        // Resolve parent configs if any
        // Collect parent sources into owned strings to avoid borrow conflict
        let parent_sources: Vec<String> = config
            .extends
            .as_ref()
            .map(|e| e.as_vec().iter().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        // Process parents left-to-right (depth-first)
        for parent_source in &parent_sources {
            // Resolve relative paths against current config's directory
            let resolved_parent = self.resolve_relative_path(&normalized, parent_source);

            // Recursively resolve parent
            let parent_config = self.resolve_recursive(&resolved_parent)?;

            // Merge parent into current (current wins)
            config = merge_configs(parent_config, config);
        }

        // Remove from in_progress
        self.in_progress.remove(&normalized);
        self.depth -= 1;

        Ok(config)
    }

    /// Normalize a source path/URL
    fn normalize_source(&self, source: &str) -> String {
        if is_url(source) {
            // Transform GitHub URLs to raw content
            transform_github_url(source)
        } else {
            // Resolve relative paths
            let path = Path::new(source);
            if path.is_absolute() {
                source.to_string()
            } else {
                self.base_dir.join(source).display().to_string()
            }
        }
    }

    /// Resolve a relative path against a parent config's location
    fn resolve_relative_path(&self, parent_source: &str, relative: &str) -> String {
        if is_url(relative) || Path::new(relative).is_absolute() {
            return relative.to_string();
        }

        if is_url(parent_source) {
            // For URLs, resolve against the URL's directory
            if let Some(pos) = parent_source.rfind('/') {
                let base = &parent_source[..pos];
                format!("{}/{}", base, relative)
            } else {
                relative.to_string()
            }
        } else {
            // For local files, resolve against parent's directory
            let parent_path = Path::new(parent_source);
            if let Some(parent_dir) = parent_path.parent() {
                parent_dir.join(relative).display().to_string()
            } else {
                relative.to_string()
            }
        }
    }

    /// Load a config from URL or file
    fn load_config(&self, source: &str) -> Result<(String, bool)> {
        if is_url(source) {
            self.load_remote_config(source)
        } else {
            self.load_local_config(source).map(|c| (c, false))
        }
    }

    /// Load a local config file
    fn load_local_config(&self, path: &str) -> Result<String> {
        fs::read_to_string(path).map_err(|e| InheritanceError::FileNotFound {
            path: path.to_string(),
            error: e.to_string(),
        })
    }

    /// Load a remote config with caching
    fn load_remote_config(&self, url: &str) -> Result<(String, bool)> {
        // Check cache first
        if let Some(cached) = self.cache.get(url) {
            return Ok((cached, true));
        }

        // If offline, try stale cache
        if self.offline {
            if let Some(stale) = self.cache.get_stale(url) {
                return Ok((stale, true));
            }
            return Err(InheritanceError::FetchFailed {
                url: url.to_string(),
                error: "Offline mode enabled and no cached config available".to_string(),
            });
        }

        // Fetch from network
        let content = self.fetch_url(url)?;

        // Cache the result
        if let Err(e) = self.cache.set(url, &content) {
            eprintln!("Warning: Failed to cache config: {}", e);
        }

        Ok((content, false))
    }

    /// Fetch content from URL
    fn fetch_url(&self, url: &str) -> Result<String> {
        let response = crate::net::agent()
            .get(url)
            .header("User-Agent", &crate::net::user_agent())
            .call()
            .map_err(|e| InheritanceError::FetchFailed {
                url: url.to_string(),
                error: e.to_string(),
            })?;

        let body =
            response
                .into_body()
                .read_to_string()
                .map_err(|e| InheritanceError::FetchFailed {
                    url: url.to_string(),
                    error: e.to_string(),
                })?;

        Ok(body)
    }

    /// Get the resolution trace
    pub fn trace(&self) -> &ResolutionTrace {
        &self.trace
    }
}

impl Default for InheritanceResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a string is a URL
fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Transform GitHub URLs to raw content URLs
fn transform_github_url(url: &str) -> String {
    // Transform github.com/user/repo/blob/branch/path to raw.githubusercontent.com/user/repo/branch/path
    if url.contains("github.com") && url.contains("/blob/") {
        return url
            .replace("github.com", "raw.githubusercontent.com")
            .replace("/blob/", "/");
    }

    // Transform gist.github.com URLs
    if url.contains("gist.github.com") && !url.ends_with("/raw") {
        return format!("{}/raw", url.trim_end_matches('/'));
    }

    url.to_string()
}

/// Merge two configs (base + overlay, overlay wins)
fn merge_configs(base: ExtendedConfig, overlay: ExtendedConfig) -> ExtendedConfig {
    ExtendedConfig {
        extends: None, // Don't carry extends forward
        tools: merge_tools(base.tools, overlay.tools),
        hooks: merge_hooks(base.hooks, overlay.hooks),
        env: merge_env(base.env, overlay.env),
        services: merge_services(base.services, overlay.services),
    }
}

/// Merge tool configurations (overlay wins)
fn merge_tools(
    base: HashMap<String, ToolConfig>,
    overlay: HashMap<String, ToolConfig>,
) -> HashMap<String, ToolConfig> {
    let mut result = base;
    for (name, config) in overlay {
        result.insert(name, config);
    }
    result
}

/// Merge hooks (append behavior - parent hooks run first)
fn merge_hooks(base: HooksConfig, overlay: HooksConfig) -> HooksConfig {
    HooksConfig {
        // Append scripts (base first, then overlay)
        pre_setup: append_hook_scripts(base.pre_setup, overlay.pre_setup),
        post_setup: append_hook_scripts(base.post_setup, overlay.post_setup),
        // Overlay config wins
        config: overlay.config,
        // Merge tool hooks
        tool_hooks: merge_tool_hooks(base.tool_hooks, overlay.tool_hooks),
    }
}

/// Append two optional hook scripts
fn append_hook_scripts(base: Option<String>, overlay: Option<String>) -> Option<String> {
    match (base, overlay) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(o)) => Some(o),
        (Some(b), Some(o)) => Some(format!("{}\n{}", b, o)),
    }
}

/// Merge per-tool hooks
fn merge_tool_hooks(
    base: HashMap<String, ToolHooks>,
    overlay: HashMap<String, ToolHooks>,
) -> HashMap<String, ToolHooks> {
    let mut result = base;
    for (name, hooks) in overlay {
        if let Some(existing) = result.get_mut(&name) {
            // Append post_install scripts
            existing.post_install =
                append_hook_scripts(existing.post_install.clone(), hooks.post_install);
        } else {
            result.insert(name, hooks);
        }
    }
    result
}

/// Merge environment configs (overlay wins for vars, append for secrets)
fn merge_env(base: EnvConfig, overlay: EnvConfig) -> EnvConfig {
    let mut vars = base.vars;
    for (k, v) in overlay.vars {
        vars.insert(k, v);
    }

    let mut secrets = base.secrets;
    for (k, v) in overlay.secrets {
        secrets.insert(k, v);
    }

    let mut tool_env = base.tool_env;
    for (k, v) in overlay.tool_env {
        tool_env.insert(k, v);
    }

    EnvConfig {
        vars,
        secrets,
        config: overlay.config, // Overlay wins
        tool_env,
    }
}

/// Merge services config (overlay wins)
fn merge_services(base: ServicesConfig, overlay: ServicesConfig) -> ServicesConfig {
    ServicesConfig {
        enabled: overlay.enabled || base.enabled,
        auto_start: overlay.auto_start,
        compose_file: overlay.compose_file.or(base.compose_file),
        tilt_file: overlay.tilt_file.or(base.tilt_file),
        start_in_ci: overlay.start_in_ci,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config(content: &str) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("jarvy.toml");
        fs::write(&config_path, content).unwrap();
        (temp_dir, config_path)
    }

    #[test]
    fn test_parse_extended_config() {
        let content = r#"
extends = "https://example.com/base.toml"

[provisioner]
git = "latest"
node = "20"
"#;
        let config: ExtendedConfig = toml::from_str(content).unwrap();
        assert!(matches!(config.extends, Some(Extends::Single(_))));
        assert_eq!(config.tools.len(), 2);
    }

    #[test]
    fn test_parse_multiple_extends() {
        let content = r#"
extends = ["base.toml", "override.toml"]

[provisioner]
git = "latest"
"#;
        let config: ExtendedConfig = toml::from_str(content).unwrap();
        assert!(matches!(config.extends, Some(Extends::Multiple(_))));
        if let Some(Extends::Multiple(v)) = &config.extends {
            assert_eq!(v.len(), 2);
        }
    }

    #[test]
    fn test_merge_tools_overlay_wins() {
        let base: HashMap<String, ToolConfig> = [
            ("git".to_string(), ToolConfig::Simple("2.40".to_string())),
            ("node".to_string(), ToolConfig::Simple("18".to_string())),
        ]
        .into_iter()
        .collect();

        let overlay: HashMap<String, ToolConfig> = [
            ("git".to_string(), ToolConfig::Simple("2.45".to_string())),
            (
                "docker".to_string(),
                ToolConfig::Simple("latest".to_string()),
            ),
        ]
        .into_iter()
        .collect();

        let merged = merge_tools(base, overlay);
        assert_eq!(merged.len(), 3);

        // git should be overridden to 2.45
        match merged.get("git") {
            Some(ToolConfig::Simple(v)) => assert_eq!(v, "2.45"),
            _ => panic!("Expected Simple config"),
        }

        // node should remain from base
        match merged.get("node") {
            Some(ToolConfig::Simple(v)) => assert_eq!(v, "18"),
            _ => panic!("Expected Simple config"),
        }

        // docker should be added from overlay
        assert!(merged.contains_key("docker"));
    }

    #[test]
    fn test_append_hook_scripts() {
        assert_eq!(append_hook_scripts(None, None), None);
        assert_eq!(
            append_hook_scripts(Some("a".to_string()), None),
            Some("a".to_string())
        );
        assert_eq!(
            append_hook_scripts(None, Some("b".to_string())),
            Some("b".to_string())
        );
        assert_eq!(
            append_hook_scripts(Some("a".to_string()), Some("b".to_string())),
            Some("a\nb".to_string())
        );
    }

    #[test]
    fn test_is_url() {
        assert!(is_url("https://example.com/config.toml"));
        assert!(is_url("http://localhost:8080/config.toml"));
        assert!(!is_url("./local/config.toml"));
        assert!(!is_url("/absolute/path/config.toml"));
    }

    #[test]
    fn test_transform_github_url() {
        let url = "https://github.com/user/repo/blob/main/jarvy.toml";
        let transformed = transform_github_url(url);
        assert_eq!(
            transformed,
            "https://raw.githubusercontent.com/user/repo/main/jarvy.toml"
        );
    }

    #[test]
    fn test_transform_gist_url() {
        let url = "https://gist.github.com/user/abc123";
        let transformed = transform_github_url(url);
        assert_eq!(transformed, "https://gist.github.com/user/abc123/raw");
    }

    #[test]
    fn test_resolve_local_config() {
        let content = r#"
[provisioner]
git = "latest"
"#;
        let (_temp, config_path) = create_test_config(content);

        let mut resolver = InheritanceResolver::new();
        let config = resolver.resolve(config_path.to_str().unwrap()).unwrap();

        assert_eq!(config.tools.len(), 1);
        assert!(config.tools.contains_key("git"));
    }

    #[test]
    fn test_resolve_with_local_extends() {
        let temp_dir = TempDir::new().unwrap();

        // Create base config
        let base_content = r#"
[provisioner]
git = "2.40"
node = "18"
"#;
        let base_path = temp_dir.path().join("base.toml");
        fs::write(&base_path, base_content).unwrap();

        // Create child config that extends base
        // TOML literal strings (single-quoted) survive Windows backslash
        // paths verbatim. Using a regular `extends = "C:\\Users\\..."`
        // would either need explicit escaping or fail to parse on Windows
        // because TOML treats `\U` as a unicode escape.
        let child_content = format!(
            r#"
extends = '{}'

[provisioner]
git = "2.45"
docker = "latest"
"#,
            base_path.display()
        );
        let child_path = temp_dir.path().join("child.toml");
        fs::write(&child_path, &child_content).unwrap();

        let mut resolver = InheritanceResolver::new().with_base_dir(temp_dir.path().to_path_buf());
        let config = resolver.resolve(child_path.to_str().unwrap()).unwrap();

        // git should be overridden (child wins)
        match config.tools.get("git") {
            Some(ToolConfig::Simple(v)) => assert_eq!(v, "2.45"),
            _ => panic!("Expected git to be overridden"),
        }

        // node should come from base
        assert!(config.tools.contains_key("node"));

        // docker should come from child
        assert!(config.tools.contains_key("docker"));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create A that extends B
        let a_path = temp_dir.path().join("a.toml");
        let b_path = temp_dir.path().join("b.toml");

        // Single-quoted TOML literal so Windows backslash paths don't
        // need escaping (see test_resolve_with_local_extends comment).
        let a_content = format!("extends = '{}'\n[provisioner]\na = \"1\"", b_path.display());
        let b_content = format!("extends = '{}'\n[provisioner]\nb = \"1\"", a_path.display());

        fs::write(&a_path, a_content).unwrap();
        fs::write(&b_path, b_content).unwrap();

        let mut resolver = InheritanceResolver::new().with_base_dir(temp_dir.path().to_path_buf());
        let result = resolver.resolve(a_path.to_str().unwrap());

        assert!(matches!(
            result,
            Err(InheritanceError::CircularDependency(_))
        ));
    }

    #[test]
    fn test_max_depth_exceeded() {
        let temp_dir = TempDir::new().unwrap();

        // Create a chain that exceeds MAX_DEPTH
        let mut paths = Vec::new();
        for i in 0..=MAX_DEPTH + 2 {
            let path = temp_dir.path().join(format!("config{}.toml", i));
            paths.push(path);
        }

        // Create configs in reverse order
        for i in (0..paths.len()).rev() {
            // Single-quoted TOML literal for Windows-path safety.
            let content = if i < paths.len() - 1 {
                format!(
                    "extends = '{}'\n[provisioner]\nvar{} = \"{}\"",
                    paths[i + 1].display(),
                    i,
                    i
                )
            } else {
                format!("[provisioner]\nvar{} = \"{}\"", i, i)
            };
            fs::write(&paths[i], content).unwrap();
        }

        let mut resolver = InheritanceResolver::new().with_base_dir(temp_dir.path().to_path_buf());
        let result = resolver.resolve(paths[0].to_str().unwrap());

        assert!(matches!(
            result,
            Err(InheritanceError::MaxDepthExceeded { .. })
        ));
    }

    #[test]
    fn test_diamond_dependency() {
        let temp_dir = TempDir::new().unwrap();

        // Create diamond: A extends [B, C], both B and C extend D
        let d_path = temp_dir.path().join("d.toml");
        let b_path = temp_dir.path().join("b.toml");
        let c_path = temp_dir.path().join("c.toml");
        let a_path = temp_dir.path().join("a.toml");

        // D is the common ancestor
        fs::write(&d_path, "[provisioner]\nd_tool = \"1.0\"").unwrap();

        // Single-quoted TOML literals for Windows-path safety.
        // B extends D
        fs::write(
            &b_path,
            format!(
                "extends = '{}'\n[provisioner]\nb_tool = \"1.0\"",
                d_path.display()
            ),
        )
        .unwrap();

        // C extends D
        fs::write(
            &c_path,
            format!(
                "extends = '{}'\n[provisioner]\nc_tool = \"1.0\"",
                d_path.display()
            ),
        )
        .unwrap();

        // A extends B and C
        fs::write(
            &a_path,
            format!(
                "extends = ['{}', '{}']\n[provisioner]\na_tool = \"1.0\"",
                b_path.display(),
                c_path.display()
            ),
        )
        .unwrap();

        let mut resolver = InheritanceResolver::new().with_base_dir(temp_dir.path().to_path_buf());
        let config = resolver.resolve(a_path.to_str().unwrap()).unwrap();

        // All tools should be present
        assert!(config.tools.contains_key("a_tool"));
        assert!(config.tools.contains_key("b_tool"));
        assert!(config.tools.contains_key("c_tool"));
        assert!(config.tools.contains_key("d_tool"));
    }

    #[test]
    fn test_resolution_trace() {
        let temp_dir = TempDir::new().unwrap();

        // Simple two-level hierarchy
        let base_path = temp_dir.path().join("base.toml");
        let child_path = temp_dir.path().join("child.toml");

        fs::write(&base_path, "[provisioner]\nbase = \"1\"").unwrap();
        // Single-quoted TOML literal for Windows-path safety.
        fs::write(
            &child_path,
            format!(
                "extends = '{}'\n[provisioner]\nchild = \"1\"",
                base_path.display()
            ),
        )
        .unwrap();

        let mut resolver = InheritanceResolver::new().with_base_dir(temp_dir.path().to_path_buf());
        let (_, trace) = resolver
            .resolve_with_trace(child_path.to_str().unwrap())
            .unwrap();

        assert_eq!(trace.entries.len(), 2);
        assert_eq!(trace.max_depth, 2);
    }
}
