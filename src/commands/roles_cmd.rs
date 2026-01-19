//! Roles command handler - manage role-based configurations

use crate::config;
use crate::roles;

/// Handle roles subcommands
pub fn run_roles(file: &str, action: &roles::RolesAction) {
    let config = config::Config::new(file);

    if let Err(e) = roles::handle_roles_command(
        action.clone(),
        Some(config.get_roles_config()),
        config
            .get_assigned_roles()
            .map(|v| v.first().copied())
            .flatten(),
    ) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
