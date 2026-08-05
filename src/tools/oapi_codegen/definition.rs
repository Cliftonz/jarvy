//! oapi-codegen - OpenAPI 3 → Go code generator
//!
//! Generates Go client and server boilerplate (chi/echo/gin/std-lib
//! flavors) from an OpenAPI 3 spec. Upstream's canonical install is a
//! plain `go install` — no brew/apt/winget package exists, so the
//! PRD-060 go fallback route is the install path on every OS.

use crate::define_tool;

define_tool!(OAPI_CODEGEN, {
    command: "oapi-codegen",
    // Ecosystem-only: upstream's README documents exactly
    // `go install github.com/oapi-codegen/oapi-codegen/v2/cmd/oapi-codegen@latest`
    // (verified 2026-08, no replace directives in go.mod). The fallback
    // runtime bootstraps go through jarvy's own registry when missing.
    fallback: { go: "github.com/oapi-codegen/oapi-codegen/v2/cmd/oapi-codegen" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oapi_codegen_registration_shape() {
        assert_eq!(OAPI_CODEGEN.command, "oapi-codegen");
        // no native package manager coverage; fallback route on all
        // platforms (verified 2026-08)
        assert!(OAPI_CODEGEN.macos.is_none());
        assert!(OAPI_CODEGEN.linux.is_none());
        assert!(OAPI_CODEGEN.windows.is_none());
        assert_eq!(OAPI_CODEGEN.fallback.len(), 1);
        assert_eq!(
            OAPI_CODEGEN.fallback[0].eco,
            crate::tools::spec::FallbackEco::Go
        );
        assert_eq!(
            OAPI_CODEGEN.fallback[0].package,
            "github.com/oapi-codegen/oapi-codegen/v2/cmd/oapi-codegen"
        );
    }
}
