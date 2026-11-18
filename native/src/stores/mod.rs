//! App stores: `Copy` structs of signals over `eq_core` state (one owner per
//! piece of state), made once in `app` and handed down by context. Each
//! store wraps an eq-core type and exposes reads as tracked getters and
//! writes as methods; screens never hold eq-core state directly.

pub mod auth;
pub mod locale;
pub mod quran;
pub mod sync;
