//! Property-based tests for Jarvy
//!
//! Uses proptest to generate arbitrary valid inputs and verify properties
//! about config parsing, version matching, and tool specifications.

mod property {
    pub mod config_properties;
    pub mod version_properties;
}
