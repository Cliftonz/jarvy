//! Doppler CLI — secrets management for services and local dev.
//!
//! Homepage: <https://www.doppler.com>. Wraps the Doppler API for
//! injecting and syncing secrets from CI + shell (`doppler run -- <cmd>`
//! injects secrets into subprocess env; `doppler secrets` for CRUD).
//!
//! ## Package coverage
//!
//! Verified 2026-08 against <https://docs.doppler.com/docs/install-cli>:
//!
//! - macOS: `brew install dopplerhq/cli/doppler` (two-slash tap shape,
//!   auto-tapped by jarvy's `install_macos()`). Docs also say to
//!   `brew install gnupg` first, but that's a dependency the formula
//!   itself declares — brew resolves it automatically, so it isn't
//!   modeled here.
//! - Windows: `winget install doppler.doppler`. A Scoop bucket also
//!   exists upstream, but winget alone already covers Windows install,
//!   so it isn't declared here too.
//! - Linux: apt / dnf / apk packages are documented but each requires a
//!   GPG-keyed repo to be added first (Doppler does not ship into
//!   standard debian / RHEL / Alpine repos). Jarvy's `linux.apt` /
//!   `linux.dnf` / `linux.apk` blocks assume the package is directly
//!   resolvable, so we OMIT them and let the runtime emit
//!   `tool.unsupported` with a link to the docs rather than silently
//!   mis-installing a name collision from the base repos (same
//!   situation as `infisical`).

use crate::define_tool;

define_tool!(DOPPLER, {
    command: "doppler",
    macos: { brew: "dopplerhq/cli/doppler" },
    windows: { winget: "doppler.doppler" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doppler_registration_shape() {
        assert_eq!(DOPPLER.command, "doppler");
        let mac = DOPPLER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("dopplerhq/cli/doppler"));
        let win = DOPPLER.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("doppler.doppler"));
        assert!(
            DOPPLER.linux.is_none(),
            "linux install requires a GPG-keyed repo add step \
             — see the module docs. Omit rather than mis-resolve."
        );
    }
}
