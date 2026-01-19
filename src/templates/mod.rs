//! Template system for Jarvy
//!
//! This module provides:
//! - Template schema definition
//! - Template loading from TOML files
//! - Built-in template catalog

pub mod builtin;
pub mod schema;

pub use builtin::{BuiltinTemplate, get_builtin_template, list_builtin_templates};
pub use schema::{Template, TemplateMeta, TemplateTools};
