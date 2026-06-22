//! `[registry]` section of `~/.jarvy/config.toml`.
//!
//! Loaded only from the global user config. Project-level `jarvy.toml`
//! never sees this section — trust-narrowing per CLAUDE.md.

use serde::{Deserialize, Serialize};

/// Default Sigstore identity-regexp expected on the manifest signature.
/// Points at the canonical `bearbinary/jarvy-tools` repo. Self-hosted
/// registries override this in the user's config.
pub const DEFAULT_IDENTITY_REGEXP: &str =
    r"^https://github\.com/bearbinary/jarvy-tools/\.github/workflows/.*\.yml@refs/heads/main$";

/// Default OIDC issuer for the GitHub Actions OIDC provider.
pub const DEFAULT_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";

/// `[registry]` section.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryConfig {
    /// HTTPS base URL of the registry. The manifest is fetched from
    /// `<url>/manifest.json` (trailing slash optional). Tool entries
    /// reference paths relative to this URL.
    pub url: String,

    /// Whether `jarvy registry sync` is enabled. Defaults to `false` so
    /// a stray `[registry] url = ...` line doesn't silently subscribe a
    /// fleet of dev machines to a third-party feed.
    #[serde(default)]
    pub enabled: bool,

    /// Sigstore identity-regexp passed to `cosign verify-blob`. Pins the
    /// workflow + repo + ref pattern that's allowed to have signed the
    /// manifest. Defaults to the canonical registry.
    #[serde(default = "default_identity_regexp")]
    pub signature_identity_regexp: String,

    /// OIDC issuer URL for the workflow's keyless cert. Defaults to GitHub
    /// Actions.
    #[serde(default = "default_oidc_issuer")]
    pub signature_oidc_issuer: String,

    /// Refuse to register tools if the manifest signature can't be
    /// verified. Default true. Set false ONLY for local development
    /// against an unsigned mirror — Jarvy emits a stderr warning every
    /// sync that it's off.
    #[serde(default = "default_true")]
    pub require_signature: bool,
}

fn default_identity_regexp() -> String {
    DEFAULT_IDENTITY_REGEXP.to_string()
}

fn default_oidc_issuer() -> String {
    DEFAULT_OIDC_ISSUER.to_string()
}

fn default_true() -> bool {
    true
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            enabled: false,
            signature_identity_regexp: DEFAULT_IDENTITY_REGEXP.to_string(),
            signature_oidc_issuer: DEFAULT_OIDC_ISSUER.to_string(),
            require_signature: true,
        }
    }
}

impl RegistryConfig {
    /// Read the `[registry]` section from `~/.jarvy/config.toml`. Returns
    /// `None` if the file or section is absent.
    pub fn load() -> Option<Self> {
        let path = crate::paths::config_toml().ok()?;
        let content = std::fs::read_to_string(&path).ok()?;

        #[derive(Deserialize)]
        struct GlobalConfig {
            registry: Option<RegistryConfig>,
        }
        let parsed: GlobalConfig = toml::from_str(&content).ok()?;
        parsed.registry
    }

    /// True if the registry is configured AND opted in.
    pub fn is_active(&self) -> bool {
        self.enabled && !self.url.is_empty()
    }

    /// Manifest URL = `<base>/manifest.json`. Tolerates a trailing slash
    /// in `url`.
    pub fn manifest_url(&self) -> String {
        let base = self.url.trim_end_matches('/');
        format!("{base}/manifest.json")
    }

    /// Build a tool-file URL by joining the relative `path` from a
    /// manifest entry onto the registry base URL.
    pub fn tool_url(&self, path: &str) -> String {
        let base = self.url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        format!("{base}/{path}")
    }

    /// Companion URL for the manifest's cosign signature — `manifest.json.sig`.
    pub fn signature_url(&self) -> String {
        format!("{}.sig", self.manifest_url())
    }

    /// Companion URL for the manifest's cosign certificate — `manifest.json.pem`.
    pub fn certificate_url(&self) -> String {
        format!("{}.pem", self.manifest_url())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_disable_registry() {
        let cfg = RegistryConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.require_signature);
        assert!(!cfg.is_active());
    }

    #[test]
    fn is_active_requires_url_and_enabled() {
        let cfg = RegistryConfig {
            enabled: true,
            ..Default::default()
        };
        assert!(!cfg.is_active(), "empty url should fail");

        let cfg = RegistryConfig {
            enabled: true,
            url: "https://example.com/registry/".into(),
            ..Default::default()
        };
        assert!(cfg.is_active());
    }

    #[test]
    fn manifest_url_normalizes_trailing_slash() {
        let cfg = RegistryConfig {
            url: "https://example.com/r/".into(),
            ..Default::default()
        };
        assert_eq!(cfg.manifest_url(), "https://example.com/r/manifest.json");

        let cfg2 = RegistryConfig {
            url: "https://example.com/r".into(),
            ..Default::default()
        };
        assert_eq!(cfg2.manifest_url(), "https://example.com/r/manifest.json");
    }

    #[test]
    fn tool_url_joins_relative_path() {
        let cfg = RegistryConfig {
            url: "https://example.com/r/".into(),
            ..Default::default()
        };
        assert_eq!(
            cfg.tool_url("tools/foo.toml"),
            "https://example.com/r/tools/foo.toml"
        );
        assert_eq!(
            cfg.tool_url("/tools/foo.toml"),
            "https://example.com/r/tools/foo.toml"
        );
    }

    #[test]
    fn cosign_companion_urls() {
        let cfg = RegistryConfig {
            url: "https://example.com/r/".into(),
            ..Default::default()
        };
        assert_eq!(
            cfg.signature_url(),
            "https://example.com/r/manifest.json.sig"
        );
        assert_eq!(
            cfg.certificate_url(),
            "https://example.com/r/manifest.json.pem"
        );
    }

    #[test]
    fn deserializes_from_toml() {
        let toml_str = r#"
            url = "https://example.com/r/"
            enabled = true
            require_signature = false
        "#;
        let cfg: RegistryConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.enabled);
        assert!(!cfg.require_signature);
        assert_eq!(cfg.url, "https://example.com/r/");
        // Defaults apply for unspecified fields.
        assert_eq!(cfg.signature_oidc_issuer, DEFAULT_OIDC_ISSUER);
    }
}
