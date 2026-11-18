//! App data directory access (PARITY B5, K1, J4, J7): where Quran DB
//! downloads live, and the bundled-asset bytes the store seeds from.
//! `nsh_core` resolves the directory per platform (`<data_dir>/<app id>`,
//! the same root `nsh.json`'s `fs` capability names `${dataDir}`); it may not
//! exist yet. Blocking file I/O happens in `eq_core::quran::store`, run
//! through `blocking(..)` by the stores.

use std::path::PathBuf;

use nsh_core::prelude::*;

/// Environment override for development: a local copy of the R2 tree
/// (`../db/quran` of the monorepo) to read Quran DBs from in place.
pub const DEV_QURAN_DIR_ENV: &str = "EQ_QURAN_DIR";

/// The app's own data directory (`${dataDir}`); `None` where the host keeps
/// no files (the web).
pub fn app_data_dir() -> Option<PathBuf> {
    data_dir()
}

/// Where downloaded Quran artifacts live: `${dataDir}/quran`.
pub fn quran_dir() -> Option<PathBuf> {
    app_data_dir().map(|dir| dir.join("quran"))
}

/// The development directory to read Quran DBs from in place, when
/// `EQ_QURAN_DIR` names an existing directory (debug builds only).
pub fn dev_quran_dir() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let dir = PathBuf::from(std::env::var_os(DEV_QURAN_DIR_ENV)?);
    dir.join("arabic").is_dir().then_some(dir)
}

/// The `asset:/` URL of a file this crate ships under `assets/` (`path`
/// relative to it), as `assets!()` publishes it.
pub fn asset_url(path: &str) -> String {
    format!("asset:/@{}/{path}", env!("CARGO_PKG_NAME"))
}

/// Reads a shipped asset's bytes (the engine answers `asset:/` URLs from
/// the host's assets; no capability needed).
pub async fn read_asset(path: &str) -> Result<Vec<u8>, String> {
    let response = fetch(asset_url(path)).await?;
    if !response.ok() {
        return Err(format!("asset {path}: status {}", response.status()));
    }
    Ok(response.body)
}
