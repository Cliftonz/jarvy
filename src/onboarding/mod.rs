//! User onboarding module for Jarvy
//!
//! This module provides:
//! - First-run detection to welcome new users
//! - Project type detection to suggest appropriate tool stacks
//! - Welcome banner with quick action suggestions

pub mod detection;
pub mod welcome;

pub use detection::{
    DetectedProject, ProjectType, detect_project_type, is_first_run, mark_initialized,
};
pub use welcome::{WelcomeBannerConfig, show_welcome_banner};
