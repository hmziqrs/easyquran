//! The signed package's delivery facts, read from the git-ignored record
//! `packages/remote-screen/scripts/delivery.sh` writes after each run.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DeliveryState {
    pub package: PackageFacts,
    pub signature: SignatureFacts,
    pub revocation: RevocationFacts,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PackageFacts {
    pub id: String,
    pub name: String,
    pub version: String,
    pub build: u64,
    pub sha256: String,
    pub bytes: u64,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SignatureFacts {
    pub release_key_id: String,
    pub root_key_id: String,
    pub certified_apps: String,
    pub cert_days: u64,
    pub min_runtime: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RevocationFacts {
    pub sequence: u64,
    pub package_revoked: bool,
    pub verify_error: String,
}

/// Where `scripts/delivery.sh` writes its record; git-ignored like the keys.
pub fn state_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packages/remote-screen/.nsh/delivery/state.json")
}

/// The record on disk, or `None` before `delivery.sh` first ran here.
pub fn read_delivery_state() -> Option<DeliveryState> {
    let raw = std::fs::read_to_string(state_path()).ok()?;
    serde_json::from_str(&raw).ok()
}
