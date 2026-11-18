//! Quran data (PARITY B1-B6): catalogue and id maps baked from web data,
//! read-only SQLite access by artifact id (never hashed, never modified),
//! surah names and mushaf divisions, view rules, the on-device artifact
//! store and the online fallback. Text is passed verbatim from database to
//! caller: no trim, no normalisation.
//!
//! Map of the web sources mirrored:
//! - [`catalogue`]: `view/source-profiles.ts`, `catalogue.ts`,
//!   `data/translations.{ts,json}`, `translation-groups.ts`
//! - [`meta`]: `data/quran-data.ts`, `mushaf-divisions.ts`,
//!   `quran-browse.ts`, `surah-names.ts`, `static/quran-meta/quran-data.json`
//! - [`db`]: `sql.ts`, `view/source-runtime.ts`, the worker's staging check
//! - [`view`]: `view/*.ts`, `text-marks.ts`
//! - [`store`]: `workers/download.ts`, `opfs-cache.ts`, `opfs-retention.ts`,
//!   `environment.ts`
//! - [`online`]: `api-client.ts`, `range-fetch.ts`, `fetch.ts`, `wire.ts`,
//!   `api-availability.ts`
//! - [`read`]: `source-plan.ts`, `script-availability.ts`, the reader's
//!   scope-to-range resolution and the local-first read ladder

pub mod catalogue;
pub mod db;
pub mod meta;
pub mod online;
pub mod read;
pub mod store;
pub mod types;
pub mod view;

pub use types::*;

#[cfg(test)]
mod conformance;
