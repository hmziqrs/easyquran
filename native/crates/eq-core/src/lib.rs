//! EasyQuran native domain logic. Plain Rust, no `nsh-*` dependency: the
//! app crate injects transport ([`net::Transport`]), storage
//! ([`kv::KeyValue`], [`kv::SecretStore`]) and time ([`clock::Clock`]), so
//! everything here runs under `cargo test -p eq-core` without an engine.
//! Rows are the PARITY.md checklist ids.

pub mod api;
pub mod audio;
pub mod clock;
pub mod device;
pub mod home;
pub mod i18n;
pub mod kv;
pub mod net;
pub mod quran;
pub mod reading;
pub mod routes;
pub mod search;
pub mod settings;
pub mod share;
pub mod store;
pub mod sync;

/// The shared merge rules (`rust/crates/eq-sync-core`), re-exported so the
/// app crate reaches them through one dependency.
pub use eq_sync_core as merge;
