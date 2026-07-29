//! Typed configuration layer.
//!
//! See [`settings::Settings`] for the fail-closed boot configuration that
//! replaces the previously-scattered `std::env::var` reads.

pub mod env;
pub mod settings;

pub use settings::{HttpSettings, ObjectStorageConfig, OptimizerConfig, Settings, SiteSettings};

/// Request body size limits (bytes) applied per route group.
pub mod body_limits {
    pub const DEFAULT: usize = 64 * 1024; // 64 KiB
    pub const POST: usize = 256 * 1024; // 256 KiB
    pub const MEDIA: usize = 2 * 1024 * 1024; // 2 MiB
}
