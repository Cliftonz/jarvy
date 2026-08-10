//! EAS CLI - Expo Application Services command-line client
//!
//! `eas` builds, submits, updates, and deploys Expo and React Native apps.
//! Expo distributes the CLI through npm and documents global installation as
//! `npm install --global eas-cli`.

use crate::define_tool;

define_tool!(EAS_CLI, {
    command: "eas",
    fallback: { npm: "eas-cli" },
    category: "mobile",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eas_cli_registration_shape() {
        assert_eq!(EAS_CLI.command, "eas");
        assert_eq!(EAS_CLI.category, Some("mobile"));
        assert!(EAS_CLI.macos.is_none());
        assert!(EAS_CLI.linux.is_none());
        assert!(EAS_CLI.windows.is_none());
        assert_eq!(EAS_CLI.fallback.len(), 1);
        assert_eq!(
            EAS_CLI.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(EAS_CLI.fallback[0].package, "eas-cli");
    }
}
