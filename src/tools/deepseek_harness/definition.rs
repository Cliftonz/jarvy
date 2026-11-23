//! DeepSeek Harness - DeepSeek's agentic coding CLI (`dsh`)

use crate::define_tool;

define_tool!(DEEPSEEK_HARNESS, {
    command: "dsh",
    // The npm fallback is the only distribution channel as of 2026-11
    // (`@deepseek-ai/dsh`, bin = `dsh`, repo deepseek-ai/deepseek-harness,
    // verified 2026-11). Upstream declares engines node >=24.9.0. Upstream
    // is a developer preview and its npm `latest` dist-tag currently
    // resolves to a release candidate.
    fallback: { npm: "@deepseek-ai/dsh" },
    category: "ai-agent",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepseek_harness_registration_shape() {
        assert_eq!(DEEPSEEK_HARNESS.command, "dsh");
        assert!(DEEPSEEK_HARNESS.macos.is_none());
        assert!(DEEPSEEK_HARNESS.linux.is_none());
        assert!(DEEPSEEK_HARNESS.windows.is_none());
        assert_eq!(DEEPSEEK_HARNESS.fallback.len(), 1);
        assert_eq!(
            DEEPSEEK_HARNESS.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(DEEPSEEK_HARNESS.fallback[0].package, "@deepseek-ai/dsh");
        assert_eq!(DEEPSEEK_HARNESS.category, Some("ai-agent"));
    }
}
