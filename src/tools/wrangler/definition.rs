//! Wrangler - Cloudflare Workers/Pages CLI
//!
//! `wrangler` is Cloudflare's developer CLI for Workers and Pages:
//! local dev server (`wrangler dev`), deploys, KV/R2/D1 bindings,
//! tail logs, and secrets management.

use crate::define_tool;

define_tool!(WRANGLER, {
    command: "wrangler",
    macos: { brew: "cloudflare-wrangler" },
    // Linux: no distro package; Linuxbrew installs the same
    // homebrew-core formula.
    linux: { brew: "cloudflare-wrangler" },
    // No first-party winget manifest as of 2026-08; the npm fallback
    // route covers Windows via `wrangler` (bin = `wrangler`,
    // verified 2026-08).
    fallback: { npm: "wrangler" },
    // Node.js CLI at runtime (dev server, deploy, tail logs all run on
    // node). Brew formula bundles node transitively; npm fallback path
    // needs it explicitly.
    depends_on_one_of: &["node", "nvm"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrangler_registration_shape() {
        assert_eq!(WRANGLER.command, "wrangler");
        let mac = WRANGLER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("cloudflare-wrangler"));
        let linux = WRANGLER.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("cloudflare-wrangler"));
        assert!(WRANGLER.windows.is_none(), "no first-party winget manifest");
        assert_eq!(WRANGLER.fallback.len(), 1);
        assert_eq!(
            WRANGLER.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(WRANGLER.fallback[0].package, "wrangler");
    }

    #[test]
    fn wrangler_needs_node_runtime() {
        assert_eq!(
            WRANGLER.depends_on_one_of,
            Some(&["node", "nvm"] as &[&str])
        );
    }
}
