//! Service seams: NSH-backed impls of `eq_core` traits plus the starter's
//! foundation seams (`http`, `auth`, `analytics`, `monitoring`). Each seam
//! is one small trait with one implementation, so swapping it is editing
//! one file. Empty modules are documented stubs owned by the area named in
//! their doc.

pub mod analytics;
pub mod audio;
pub mod auth;
pub mod files;
pub mod http;
pub mod monitoring;
pub mod notifications;
pub mod storage;
pub mod transport;
