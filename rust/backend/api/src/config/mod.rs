pub mod env;
pub mod settings;

pub use settings::{
    HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, RateLimitSettings, Settings,
    SiteSettings,
};

pub mod body_limits {
    pub const DEFAULT: usize = 64 * 1024;
    pub const POST: usize = 256 * 1024;
    pub const MEDIA: usize = 2 * 1024 * 1024;
}
