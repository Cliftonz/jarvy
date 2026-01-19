//! CLI argument parsing and command definitions for Jarvy
//!
//! This module contains all CLI-related types extracted from main.rs:
//! - `Cli` struct with clap derive macros
//! - `Commands` enum with all subcommands
//! - `OutputFormat` enum for output formatting
//! - Subcommand enums for nested commands
//! - Helper functions for argument parsing

mod args;
mod subcommands;

pub use args::*;
pub use subcommands::*;
