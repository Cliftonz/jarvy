//! Environment variables management module
//!
//! This module provides functionality for:
//! - Variable expansion ($HOME, $PWD, $USER, etc.)
//! - .env file generation
//! - Shell rc file modification
//! - Secret prompting with hidden input

mod dotenv;
mod expand;
mod secrets;
mod shell;

pub use dotenv::{DotenvConfig, DotenvError, generate_dotenv, preview_dotenv};
pub use expand::{EnvContext, expand_value};
pub use secrets::{SecretError, SecretsConfig, collect_secrets};
pub use shell::{
    ShellConfig, ShellError, ShellType, detect_shell, get_rc_path, parse_shell, preview_shell_rc,
    update_shell_rc,
};
