//! Audio logic (PARITY D1, D2, D4): recitation catalogue, URL building, play
//! policy, queue and repeat rules, saved session, offline audio records.
//! Ports of web `lib/audio/*.ts`. No playback here; the app drives NSH audio.

pub mod catalogue;
pub mod offline;
pub mod policy;
pub mod queue;
pub mod repeat;
pub mod session;
pub mod url;
