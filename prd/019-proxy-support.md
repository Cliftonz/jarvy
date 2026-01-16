# PRD-019: Corporate Proxy and Network Configuration Support

## Overview

Enable Jarvy to work seamlessly in corporate environments that require HTTP/HTTPS/SOCKS proxies, custom CA certificates, and network-specific configurations.

## Problem Statement

Many enterprise developers work behind corporate proxies that:
1. Intercept and inspect HTTPS traffic (MITM proxies)
2. Require authentication for internet access
3. Use custom CA certificate bundles
4. Have allowlists/denylists for specific domains

Currently, Jarvy assumes direct internet connectivity. This blocks adoption in ~40% of enterprise environments where developers must manually configure proxy settings for each tool, leading to:
- Failed package manager operations (brew, apt, npm)
- Git clone/fetch failures
- Custom install script timeouts
- Inconsistent tool behavior

## Evidence

- Common enterprise friction: "Proxy not working with brew/npm/git"
- Competitors (nix, devcontainers) all support proxy configuration
- Package managers require explicit proxy configuration
- MITM proxies break TLS unless custom CA certs are trusted

## Requirements

### Functional Requirements

1. **Environment variable support**: Respect HTTP_PROXY, HTTPS_PROXY, NO_PROXY
2. **Config file proxy settings**: Define proxy in jarvy.toml `[network]` section
3. **SOCKS5 proxy support**: Support SOCKS5 proxies for all operations
4. **Custom CA certificates**: Configure CA bundle for MITM proxy environments
5. **Proxy authentication**: Support basic auth (username:password)
6. **Per-tool overrides**: Allow specific tools to use different proxy settings
7. **CLI configuration**: `jarvy config proxy` command for interactive setup
8. **Proxy propagation**: Pass settings to curl, wget, git, package managers

### Non-Functional Requirements

1. Proxy credentials never logged in plain text
2. Settings persist across sessions
3. Environment variables take precedence over config file
4. Validate proxy connectivity before operations
5. Clear error messages when proxy blocks requests

## User Stories

### US-1: Environment Variable Support
**As a** developer in a corporate environment,
**I want** Jarvy to respect standard proxy environment variables (HTTP_PROXY, HTTPS_PROXY, NO_PROXY),
**So that** I can use existing proxy configuration without additional setup.

### US-2: Config File Proxy Settings
**As a** team lead,
**I want** to define proxy settings in jarvy.toml under a `[network]` section,
**So that** my team can share consistent proxy configuration via version control.

### US-3: SOCKS5 Proxy Support
**As a** developer behind a corporate SOCKS5 proxy,
**I want** Jarvy to route all network traffic through my SOCKS5 proxy,
**So that** I can install tools and download packages.

### US-4: Custom CA Certificate Bundles
**As a** developer in an environment with MITM proxies,
**I want** to configure a custom CA certificate bundle,
**So that** TLS connections through the proxy are trusted.

### US-5: Proxy Authentication
**As a** developer with an authenticated proxy,
**I want** to provide proxy credentials (username/password),
**So that** Jarvy can authenticate with the corporate proxy.

### US-6: Per-Tool Proxy Overrides
**As a** developer,
**I want** to configure different proxy settings for specific tools,
**So that** I can bypass the proxy for internal resources or use different proxies for different tools.

### US-7: Interactive Proxy Configuration
**As a** developer setting up Jarvy for the first time,
**I want** to use `jarvy config proxy` to interactively configure proxy settings,
**So that** I can easily set up proxy without editing config files.

## Proposed Config Syntax

```toml
# jarvy.toml

[tools]
node = "20"
docker = "latest"

# Global network configuration
[network]
# HTTP/HTTPS proxy (can use environment variables via $VAR syntax)
http_proxy = "http://proxy.corp.example.com:8080"
https_proxy = "http://proxy.corp.example.com:8080"

# SOCKS5 proxy (alternative to HTTP proxy)
# socks_proxy = "socks5://proxy.corp.example.com:1080"

# Hosts to bypass proxy (comma-separated or array)
no_proxy = ["localhost", "127.0.0.1", "*.internal.corp", "10.0.0.0/8"]

# Proxy authentication
[network.auth]
username = "jdoe"
password = { prompt = "Enter proxy password", hidden = true }
# Or reference a file
# password = { from_file = "~/.proxy-password" }
# Or use environment variable
# password = { env = "PROXY_PASSWORD" }

# Custom CA certificate bundle for MITM proxies
[network.tls]
ca_bundle = "/etc/ssl/certs/corporate-ca-bundle.crt"
# Or append to system certs
# ca_cert = "/etc/ssl/certs/corporate-root-ca.crt"
# Disable TLS verification (NOT RECOMMENDED, use only for testing)
# insecure = true

# Per-tool proxy overrides
[network.overrides.git]
# Git uses a different internal proxy
https_proxy = "http://git-proxy.corp.example.com:8888"

[network.overrides.docker]
# Docker registry goes direct
no_proxy = ["*.docker.io", "registry-1.docker.io"]

[network.overrides.npm]
# NPM uses internal registry, no proxy needed
https_proxy = ""  # Empty string = no proxy
```

## Technical Approach

### Proxy Configuration Types

```rust
// src/network/mod.rs
#[derive(Deserialize, Default)]
pub struct NetworkConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub socks_proxy: Option<String>,
    pub no_proxy: Option<NoProxy>,
    pub auth: Option<ProxyAuth>,
    pub tls: Option<TlsConfig>,
    pub overrides: HashMap<String, NetworkOverride>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum NoProxy {
    String(String),      // Comma-separated
    Array(Vec<String>),  // Array format
}

#[derive(Deserialize)]
pub struct ProxyAuth {
    pub username: String,
    pub password: PasswordSource,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum PasswordSource {
    Plain(String),
    Prompt { prompt: String, hidden: Option<bool> },
    File { from_file: PathBuf },
    Env { env: String },
}

#[derive(Deserialize)]
pub struct TlsConfig {
    pub ca_bundle: Option<PathBuf>,
    pub ca_cert: Option<PathBuf>,
    pub insecure: Option<bool>,
}

#[derive(Deserialize, Default)]
pub struct NetworkOverride {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub no_proxy: Option<NoProxy>,
}
```

### Environment Variable Resolution

```rust
// src/network/env.rs
use std::env;

pub struct ProxyResolver {
    config: NetworkConfig,
}

impl ProxyResolver {
    /// Resolve effective proxy settings for a given tool
    /// Priority: 1. Environment vars, 2. Tool override, 3. Global config
    pub fn resolve_for_tool(&self, tool: &str) -> EffectiveProxy {
        // Check environment variables first
        let http_proxy = env::var("HTTP_PROXY")
            .or_else(|_| env::var("http_proxy"))
            .ok()
            .or_else(|| self.get_tool_override(tool, |o| o.http_proxy.clone()))
            .or_else(|| self.config.http_proxy.clone());

        let https_proxy = env::var("HTTPS_PROXY")
            .or_else(|_| env::var("https_proxy"))
            .ok()
            .or_else(|| self.get_tool_override(tool, |o| o.https_proxy.clone()))
            .or_else(|| self.config.https_proxy.clone());

        let no_proxy = env::var("NO_PROXY")
            .or_else(|_| env::var("no_proxy"))
            .ok()
            .map(|s| NoProxy::String(s))
            .or_else(|| self.get_tool_override(tool, |o| o.no_proxy.clone()))
            .or_else(|| self.config.no_proxy.clone());

        EffectiveProxy {
            http_proxy,
            https_proxy,
            no_proxy,
            socks_proxy: self.config.socks_proxy.clone(),
            auth: self.resolve_auth(),
            ca_bundle: self.config.tls.as_ref().and_then(|t| t.ca_bundle.clone()),
        }
    }

    fn resolve_auth(&self) -> Option<(String, String)> {
        let auth = self.config.auth.as_ref()?;
        let password = match &auth.password {
            PasswordSource::Plain(p) => p.clone(),
            PasswordSource::Env { env } => std::env::var(env).ok()?,
            PasswordSource::File { from_file } => {
                std::fs::read_to_string(from_file).ok()?.trim().to_string()
            }
            PasswordSource::Prompt { prompt, hidden } => {
                if *hidden.as_ref().unwrap_or(&true) {
                    rpassword::prompt_password(prompt).ok()?
                } else {
                    dialoguer::Input::new().with_prompt(prompt).interact_text().ok()?
                }
            }
        };
        Some((auth.username.clone(), password))
    }
}
```

### Proxy Propagation to Tools

```rust
// src/network/propagate.rs
use std::collections::HashMap;
use std::process::Command;

impl EffectiveProxy {
    /// Get environment variables to set for child processes
    pub fn as_env_vars(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if let Some(ref http) = self.http_proxy {
            let proxy_url = self.inject_auth(http);
            env.insert("HTTP_PROXY".into(), proxy_url.clone());
            env.insert("http_proxy".into(), proxy_url);
        }

        if let Some(ref https) = self.https_proxy {
            let proxy_url = self.inject_auth(https);
            env.insert("HTTPS_PROXY".into(), proxy_url.clone());
            env.insert("https_proxy".into(), proxy_url);
        }

        if let Some(ref no_proxy) = self.no_proxy {
            let value = no_proxy.to_string();
            env.insert("NO_PROXY".into(), value.clone());
            env.insert("no_proxy".into(), value);
        }

        // Tool-specific variables
        if let Some(ref ca) = self.ca_bundle {
            let ca_path = ca.to_string_lossy().to_string();
            // curl
            env.insert("CURL_CA_BUNDLE".into(), ca_path.clone());
            // wget
            env.insert("SSL_CERT_FILE".into(), ca_path.clone());
            // Node.js
            env.insert("NODE_EXTRA_CA_CERTS".into(), ca_path.clone());
            // Python requests
            env.insert("REQUESTS_CA_BUNDLE".into(), ca_path.clone());
            // Git
            env.insert("GIT_SSL_CAINFO".into(), ca_path);
        }

        env
    }

    /// Apply proxy settings to a Command
    pub fn apply_to_command(&self, cmd: &mut Command) {
        for (key, value) in self.as_env_vars() {
            cmd.env(key, value);
        }
    }

    fn inject_auth(&self, proxy_url: &str) -> String {
        if let Some((user, pass)) = &self.auth {
            // Parse URL and inject credentials
            if let Ok(mut url) = url::Url::parse(proxy_url) {
                let _ = url.set_username(user);
                let _ = url.set_password(Some(pass));
                return url.to_string();
            }
        }
        proxy_url.to_string()
    }
}
```

### Package Manager Propagation

```rust
// src/network/package_managers.rs

/// Configure git to use proxy
pub fn configure_git_proxy(proxy: &EffectiveProxy) -> Result<(), NetworkError> {
    if let Some(ref https) = proxy.https_proxy {
        let proxy_url = proxy.inject_auth(https);
        Command::new("git")
            .args(["config", "--global", "http.proxy", &proxy_url])
            .status()?;
        Command::new("git")
            .args(["config", "--global", "https.proxy", &proxy_url])
            .status()?;
    }

    if let Some(ref ca) = proxy.ca_bundle {
        Command::new("git")
            .args(["config", "--global", "http.sslCAInfo", &ca.to_string_lossy()])
            .status()?;
    }

    Ok(())
}

/// Configure npm to use proxy
pub fn configure_npm_proxy(proxy: &EffectiveProxy) -> Result<(), NetworkError> {
    if let Some(ref https) = proxy.https_proxy {
        let proxy_url = proxy.inject_auth(https);
        Command::new("npm")
            .args(["config", "set", "proxy", &proxy_url])
            .status()?;
        Command::new("npm")
            .args(["config", "set", "https-proxy", &proxy_url])
            .status()?;
    }

    if let Some(ref ca) = proxy.ca_bundle {
        Command::new("npm")
            .args(["config", "set", "cafile", &ca.to_string_lossy()])
            .status()?;
    }

    Ok(())
}

/// Homebrew respects standard env vars, but we can set additional ones
pub fn homebrew_env_vars(proxy: &EffectiveProxy) -> HashMap<String, String> {
    let mut env = proxy.as_env_vars();
    // Homebrew-specific CA bundle
    if let Some(ref ca) = proxy.ca_bundle {
        env.insert("HOMEBREW_CA_BUNDLE".into(), ca.to_string_lossy().to_string());
    }
    env
}

/// APT requires /etc/apt/apt.conf.d/ configuration (needs sudo)
pub fn apt_proxy_config(proxy: &EffectiveProxy) -> String {
    let mut config = String::new();
    if let Some(ref http) = proxy.http_proxy {
        config.push_str(&format!("Acquire::http::Proxy \"{}\";\n", http));
    }
    if let Some(ref https) = proxy.https_proxy {
        config.push_str(&format!("Acquire::https::Proxy \"{}\";\n", https));
    }
    config
}
```

### CLI Commands

```rust
// src/main.rs additions

#[derive(Subcommand)]
enum ConfigCommand {
    /// Configure proxy settings interactively
    Proxy {
        /// Show current proxy configuration
        #[arg(long)]
        show: bool,

        /// Clear proxy configuration
        #[arg(long)]
        clear: bool,

        /// Test proxy connectivity
        #[arg(long)]
        test: bool,

        /// Set HTTP proxy URL
        #[arg(long)]
        http: Option<String>,

        /// Set HTTPS proxy URL
        #[arg(long)]
        https: Option<String>,

        /// Set SOCKS5 proxy URL
        #[arg(long)]
        socks: Option<String>,

        /// Set no-proxy hosts (comma-separated)
        #[arg(long)]
        no_proxy: Option<String>,

        /// Set CA certificate bundle path
        #[arg(long)]
        ca_bundle: Option<PathBuf>,
    },
}
```

## CLI Usage Examples

```bash
# Interactive proxy configuration
jarvy config proxy
# Prompts for:
#   HTTP Proxy URL: http://proxy.corp.com:8080
#   HTTPS Proxy URL: [same as HTTP]
#   Proxy Username (optional): jdoe
#   Proxy Password: ****
#   No-proxy hosts: localhost,127.0.0.1,*.internal.corp
#   CA Bundle path (optional): /etc/ssl/certs/corp-ca.crt

# Set proxy via CLI flags
jarvy config proxy --http http://proxy:8080 --https http://proxy:8080

# Set SOCKS5 proxy
jarvy config proxy --socks socks5://proxy:1080

# Configure CA bundle for MITM proxy
jarvy config proxy --ca-bundle /etc/ssl/certs/corporate-ca-bundle.crt

# Show current proxy configuration
jarvy config proxy --show

# Test proxy connectivity
jarvy config proxy --test

# Clear proxy configuration
jarvy config proxy --clear

# Normal setup (proxy settings applied automatically)
jarvy setup
```

## Proxy Propagation Matrix

| Component | HTTP_PROXY | HTTPS_PROXY | NO_PROXY | CA Bundle | Notes |
|-----------|------------|-------------|----------|-----------|-------|
| curl | via env | via env | via env | CURL_CA_BUNDLE | Standard env vars |
| wget | via env | via env | via env | SSL_CERT_FILE | Standard env vars |
| git | via env or git config | via env or git config | via env | GIT_SSL_CAINFO | Also http.proxy git config |
| Homebrew | via env | via env | via env | HOMEBREW_CA_BUNDLE | Custom var for CA |
| apt | apt.conf.d file | apt.conf.d file | - | /etc/ssl/certs | Needs sudo |
| dnf/yum | /etc/yum.conf | /etc/yum.conf | - | sslcacert= | Config file |
| npm | npm config | npm config | via env | cafile config | Persistent config |
| pip | via env | via env | via env | REQUESTS_CA_BUNDLE | Standard env vars |
| Docker | daemon.json | daemon.json | - | daemon.json | Daemon restart needed |
| Custom scripts | via env | via env | via env | tool-specific | Passed as env vars |

## Testing Proxy Configuration

```bash
# Test proxy connectivity and certificate trust
jarvy config proxy --test

# Output:
# Testing proxy configuration...
#   HTTP Proxy: http://proxy.corp.com:8080 ... OK
#   HTTPS Proxy: http://proxy.corp.com:8080 ... OK
#   CA Bundle: /etc/ssl/certs/corp-ca.crt ... Valid
#   Test URLs:
#     https://github.com ... OK (200)
#     https://registry.npmjs.org ... OK (200)
#     https://raw.githubusercontent.com ... OK (200)
# Proxy configuration is working correctly.

# If proxy auth fails:
# Testing proxy configuration...
#   HTTP Proxy: http://proxy.corp.com:8080 ... FAILED
#   Error: Proxy authentication required (407)
#   Hint: Configure proxy authentication with:
#     jarvy config proxy --auth
```

## Implementation Steps

1. Add `NetworkConfig` parsing to `src/config.rs`
2. Create `src/network/mod.rs` module structure
3. Implement proxy resolution with environment variable precedence
4. Implement credential resolution (prompt, file, env)
5. Create proxy propagation for child processes
6. Add package manager-specific configuration helpers
7. Implement `jarvy config proxy` CLI command
8. Add proxy connectivity testing
9. Integrate proxy settings into tool installation flow
10. Write unit and integration tests
11. Update documentation

## Acceptance Criteria

1. **Environment Variables**: Jarvy respects HTTP_PROXY, HTTPS_PROXY, NO_PROXY env vars
2. **Config File**: `[network]` section in jarvy.toml is parsed and applied
3. **SOCKS5**: SOCKS5 proxies work for all network operations
4. **CA Certificates**: Custom CA bundles are propagated to all tools
5. **Authentication**: Basic auth credentials can be configured (securely)
6. **Per-Tool Overrides**: Tool-specific proxy settings work correctly
7. **CLI Command**: `jarvy config proxy` works for configuration
8. **Propagation**: Proxy settings propagate to curl, wget, git, brew, apt, npm
9. **Testing**: `jarvy config proxy --test` validates proxy connectivity
10. **Credentials Security**: Proxy passwords are never logged in plain text

## Non-Goals

- **VPN configuration**: Jarvy does not configure VPN connections
- **PAC file support**: Complex PAC (Proxy Auto-Configuration) files are not supported
- **Proxy server management**: Jarvy does not run or manage proxy servers
- **Network diagnostics**: Beyond basic connectivity testing
- **System-wide proxy**: Only affects Jarvy operations, not system settings

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Enterprise environment support | ~60% | 95%+ |
| Manual proxy configuration | Required | Automated |
| Proxy-related support tickets | High | Low |
| Tool installation behind proxy | Often fails | Reliable |

## Risks

1. **Platform-specific configuration**: Different package managers have different proxy config methods
   - Mitigation: Document platform-specific behaviors, test on all platforms
2. **Credential exposure**: Proxy passwords could be logged
   - Mitigation: Redact credentials in logs, warn about plain-text passwords
3. **MITM certificate issues**: Custom CA certs may not work with all tools
   - Mitigation: Document CA propagation, provide troubleshooting guide
4. **Breaking existing setups**: New proxy handling could conflict with existing config
   - Mitigation: Environment variables always take precedence

## Dependencies

- `url` - URL parsing for credential injection
- `rpassword` - Hidden password input (may already exist)
- `dialoguer` - Interactive prompts (may already exist)
- `reqwest` - For proxy connectivity testing

## Effort Estimate

- Config parsing: 0.5 days
- Proxy resolution: 1 day
- Credential handling: 0.5 days
- Propagation to tools: 1.5 days
- Package manager helpers: 1 day
- CLI commands: 0.5 days
- Connectivity testing: 0.5 days
- Integration: 1 day
- Testing: 1 day
- Documentation: 0.5 days

**Total: ~8 days**

## Files to Create/Modify

- `src/network/mod.rs` - New module
- `src/network/config.rs` - NetworkConfig types
- `src/network/resolve.rs` - Proxy resolution
- `src/network/propagate.rs` - Environment propagation
- `src/network/package_managers.rs` - PM-specific config
- `src/network/test.rs` - Connectivity testing
- `src/config.rs` - Add NetworkConfig parsing
- `src/main.rs` - Add `config proxy` command
- `src/tools/common.rs` - Apply proxy to tool commands
- `Cargo.toml` - Add dependencies if needed
