//! The account device list (devices.md D2). One row per (user, client-minted id).
//! Rows never hold raw UA, screen, model or IP; closed enums are validated in
//! `device_v1` before they reach here.

mod actions;
mod model;
mod slice;

pub use actions::*;
pub use model::*;
pub use slice::*;
