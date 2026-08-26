//! Appium - cross-platform mobile application automation server
//!
//! Appium is distributed through npm. The default post-install hook adds the
//! official mobile preset so a fresh install includes the UIAutomator2 and
//! Espresso Android drivers (plus XCUITest when running on macOS).

use crate::define_tool;

define_tool!(APPIUM, {
    command: "appium",
    fallback: { npm: "appium" },
    default_hook: {
        description: "Install Appium mobile drivers and plugins",
        script: "appium setup mobile"
    },
    // Node.js server at runtime. Matches the cypress / playwright
    // pattern — either the runtime itself or nvm satisfies the check.
    depends_on_one_of: &["node", "nvm"],
    category: "mobile",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appium_registration_shape() {
        assert_eq!(APPIUM.command, "appium");
        assert_eq!(APPIUM.category, Some("mobile"));
        assert!(APPIUM.macos.is_none());
        assert!(APPIUM.linux.is_none());
        assert!(APPIUM.windows.is_none());
        assert_eq!(APPIUM.fallback.len(), 1);
        assert_eq!(APPIUM.fallback[0].eco, crate::tools::spec::FallbackEco::Npm);
        assert_eq!(APPIUM.fallback[0].package, "appium");
        let hook = APPIUM.default_hook.as_ref().expect("mobile setup hook");
        assert_eq!(
            hook.description,
            "Install Appium mobile drivers and plugins"
        );
        assert_eq!(hook.script, "appium setup mobile");
    }

    #[test]
    fn appium_needs_node_runtime() {
        assert_eq!(APPIUM.depends_on_one_of, Some(&["node", "nvm"] as &[&str]));
    }
}
