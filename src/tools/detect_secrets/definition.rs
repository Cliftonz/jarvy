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
    fn detect_secrets_registration_shape() {
        assert_eq!(DETECT_SECRETS.command, "detect-secrets");
        let mac = DETECT_SECRETS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("detect-secrets"));
    }
}
