//! App state owned by the foundation area: the event bus, the background
//! task manager and the undo/redo history type. Everything public here is
//! a cross-area contract; the docs of each item are its spec.
//!
//! Deliberately absent: appearance and locale signals. NSH's own reads
//! (`dark()`, `locale()`, `font_scale()`) are reactive and follow the
//! platform natively; copying them into app signals would stop that
//! following.

pub mod event_bus;
pub mod history;
pub mod tasks;
