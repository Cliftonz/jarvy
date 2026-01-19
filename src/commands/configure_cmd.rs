//! Configure command handler - generate default jarvy.toml

use crate::config::create_default_config;

/// Run the configure command
pub fn run_configure() {
    create_default_config();
}
