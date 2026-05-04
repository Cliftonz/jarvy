//! detect-secrets - enterprise secret detection
//!
//! detect-secrets is an enterprise-friendly tool by Yelp for detecting and
//! preventing secrets in code using a baseline approach.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DETECT_SECRETS, {
    command: "detect-secrets",
    macos: { brew: "detect-secrets" },
    linux: { brew: "detect-secrets" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_detect_secrets_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
