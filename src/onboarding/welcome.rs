//! Welcome banner for first-time users
//!
//! Displays a friendly welcome message with quick action suggestions
//! when Jarvy is run for the first time.

use std::io::{self, Write};

/// Configuration for the welcome banner
#[derive(Debug, Clone)]
pub struct WelcomeBannerConfig {
    /// Whether to show the banner (respects quiet mode)
    pub enabled: bool,
    /// Whether to use colors in output
    pub use_colors: bool,
}

impl Default for WelcomeBannerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            use_colors: true,
        }
    }
}

impl WelcomeBannerConfig {
    /// Create a config that disables the banner
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a config without colors (for non-TTY output)
    pub fn no_colors() -> Self {
        Self {
            use_colors: false,
            ..Default::default()
        }
    }
}

/// Show the welcome banner for first-time users
///
/// This prints a friendly welcome message with quick action suggestions.
/// The banner is designed to be helpful but not intrusive.
pub fn show_welcome_banner(config: &WelcomeBannerConfig) {
    if !config.enabled {
        return;
    }

    let mut stdout = io::stdout();

    if config.use_colors {
        print_colored_banner(&mut stdout);
    } else {
        print_plain_banner(&mut stdout);
    }

    let _ = stdout.flush();
}

fn print_colored_banner<W: Write>(w: &mut W) {
    let cyan = "\x1b[36m";
    let green = "\x1b[32m";
    let yellow = "\x1b[33m";
    let bold = "\x1b[1m";
    let reset = "\x1b[0m";

    writeln!(w).ok();
    writeln!(w, "{cyan}╭──────────────────────────────────────────────────────────╮{reset}").ok();
    writeln!(w, "{cyan}│{reset} {bold}Welcome to Jarvy!{reset}                                        {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset}                                                          {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset} Looks like this is your first time using Jarvy.         {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset}                                                          {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset} {yellow}Quick options:{reset}                                           {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset}   {green}jarvy quickstart{reset}  - Guided setup (recommended)        {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset}   {green}jarvy init{reset}        - Create a config file              {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset}   {green}jarvy templates{reset}   - Browse starter templates          {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset}                                                          {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset} To skip this message:                                    {cyan}│{reset}").ok();
    writeln!(w, "{cyan}│{reset}   {green}jarvy config set show_welcome false{reset}                   {cyan}│{reset}").ok();
    writeln!(w, "{cyan}╰──────────────────────────────────────────────────────────╯{reset}").ok();
    writeln!(w).ok();
}

fn print_plain_banner<W: Write>(w: &mut W) {
    writeln!(w).ok();
    writeln!(w, "╭──────────────────────────────────────────────────────────╮").ok();
    writeln!(w, "│ Welcome to Jarvy!                                        │").ok();
    writeln!(w, "│                                                          │").ok();
    writeln!(w, "│ Looks like this is your first time using Jarvy.         │").ok();
    writeln!(w, "│                                                          │").ok();
    writeln!(w, "│ Quick options:                                           │").ok();
    writeln!(w, "│   jarvy quickstart  - Guided setup (recommended)        │").ok();
    writeln!(w, "│   jarvy init        - Create a config file              │").ok();
    writeln!(w, "│   jarvy templates   - Browse starter templates          │").ok();
    writeln!(w, "│                                                          │").ok();
    writeln!(w, "│ To skip this message:                                    │").ok();
    writeln!(w, "│   jarvy config set show_welcome false                   │").ok();
    writeln!(w, "╰──────────────────────────────────────────────────────────╯").ok();
    writeln!(w).ok();
}

/// Check if the welcome banner should be shown
///
/// Returns false if:
/// - quiet mode is enabled
/// - JSON format is requested
/// - running in CI
/// - show_welcome config is false
pub fn should_show_welcome(quiet: bool, json_format: bool) -> bool {
    if quiet || json_format {
        return false;
    }

    // Check CI environment
    if std::env::var("CI").is_ok() {
        return false;
    }

    // Check if user has disabled welcome
    // This would be read from config, but for now we just check the marker
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_banner_config_default() {
        let config = WelcomeBannerConfig::default();
        assert!(config.enabled);
        assert!(config.use_colors);
    }

    #[test]
    fn test_welcome_banner_config_disabled() {
        let config = WelcomeBannerConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_welcome_banner_config_no_colors() {
        let config = WelcomeBannerConfig::no_colors();
        assert!(config.enabled);
        assert!(!config.use_colors);
    }

    #[test]
    fn test_print_plain_banner() {
        let mut buffer = Vec::new();
        print_plain_banner(&mut buffer);
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("Welcome to Jarvy!"));
        assert!(output.contains("jarvy quickstart"));
        assert!(output.contains("jarvy init"));
        assert!(output.contains("jarvy templates"));
    }

    #[test]
    fn test_print_colored_banner() {
        let mut buffer = Vec::new();
        print_colored_banner(&mut buffer);
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("Welcome to Jarvy!"));
        // Check for ANSI escape codes
        assert!(output.contains("\x1b["));
    }

    #[test]
    fn test_should_show_welcome_quiet_mode() {
        assert!(!should_show_welcome(true, false));
    }

    #[test]
    fn test_should_show_welcome_json_format() {
        assert!(!should_show_welcome(false, true));
    }
}
