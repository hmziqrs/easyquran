//! Device identity (PARITY I8, G2; contracts/devices.md D3): per-install id,
//! P-256 device key in a [`crate::kv::SecretStore`], challenge + register
//! (`device/v1/*`), continuity hint, and the client/platform/key-store
//! labels the backend accepts.

pub mod identity;
pub mod keys;
pub mod register;

pub use identity::{
    DeviceEnv, FormFactor, Platform, check_registration_shape, client_label, device_id_for,
    key_store_label,
};
pub use keys::DeviceKey;
pub use register::{RegisterOutcome, ensure_device_registered, registration_due};
