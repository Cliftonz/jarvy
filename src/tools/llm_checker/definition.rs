//! llm-checker - hardware scanner and local LLM recommendation CLI
//!
//! `llm-checker` inspects the host's CPU, RAM, and GPU to recommend which
//! local LLMs it can realistically run. It has no native package-manager
//! listing and is distributed only via `npm install -g llm-checker`.

use crate::define_tool;

define_tool!(LLM_CHECKER, {
    command: "llm-checker",
    fallback: { npm: "llm-checker" },
    category: "ai-agent",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_checker_registration_shape() {
        assert_eq!(LLM_CHECKER.command, "llm-checker");
        assert_eq!(LLM_CHECKER.category, Some("ai-agent"));
        assert!(LLM_CHECKER.macos.is_none());
        assert!(LLM_CHECKER.linux.is_none());
        assert!(LLM_CHECKER.windows.is_none());
        assert_eq!(LLM_CHECKER.fallback.len(), 1);
        assert_eq!(
            LLM_CHECKER.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(LLM_CHECKER.fallback[0].package, "llm-checker");
    }
}
