//! structurizr - Software architecture models as code (C4 model)
//!
//! The consolidated Structurizr tool (2026 vNext) replaces the three former
//! products — the CLI, Structurizr Lite, and the on-premises install — with a
//! single Java application exposing four modes:
//!   - `structurizr export`   — render a `.dsl`/JSON workspace to PlantUML etc. (former CLI)
//!   - `structurizr local`    — local authoring server (former Structurizr Lite)
//!   - `structurizr server`   — on-premises server (former on-premises install)
//!   - `structurizr playground` — standalone DSL playground
//!
//! The old `structurizr-cli` Homebrew formula is deprecated (upstream repo
//! archived) — this points at the new `structurizr` formula instead.
//!
//! Java-based (the brew formula pulls its own `openjdk`), so — like `allure` —
//! we don't declare a `depends_on: ["java"]`; Homebrew resolves the JRE.

use crate::define_tool;

define_tool!(STRUCTURIZR, {
    command: "structurizr",
    macos: { brew: "structurizr" },
    // Linux: no distro package; homebrew-core ships an architecture-independent
    // bottle that Linuxbrew installs. Otherwise run the `.war` via `java -jar`
    // or the `structurizr/structurizr` Docker image.
    linux: { brew: "structurizr" },
    // No first-party winget/choco manifest as of 2026-07; on Windows run the
    // Java `.war` (https://github.com/structurizr/structurizr) or Docker image.
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structurizr_registration_shape() {
        assert_eq!(STRUCTURIZR.command, "structurizr");
        let mac = STRUCTURIZR.macos.expect("structurizr must support macOS");
        assert_eq!(mac.brew, Some("structurizr"));
        let linux = STRUCTURIZR.linux.expect("structurizr must support Linux");
        assert_eq!(linux.brew, Some("structurizr"));
        assert!(
            STRUCTURIZR.windows.is_none(),
            "no first-party winget manifest; use the .war or Docker image"
        );
    }
}
