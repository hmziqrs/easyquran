//! Account device registration (contracts/devices.md D2, D3; web
//! `register.ts`). Sign-in and push-token refresh trigger it; at most one
//! register per device per 24 h; a failure never blocks reading or sync.
//!
//! With a usable key store the register carries the public JWK and a
//! signature over a fresh session nonce (`POST /device/v1/challenge`), plus
//! a continuity hint on Android, Windows and Linux. Without one it goes out
//! with `key_store: none`. A 409 means the id was removed or its key
//! conflicts: a new id and key are minted and the register runs once more.
//! The server's `device_id` always wins.
//!
//! The private key lives in the [`SecretStore`] only; nothing here logs the
//! id, the key, the nonce or the signature.

use serde::{Deserialize, Serialize};

use super::{
    identity::{
        DeviceEnv, check_registration_shape, client_label, device_id_for, key_store_label,
        new_device_id, set_device_id_for,
    },
    keys::{DeviceKey, b64u_decode, continuity_hint, proof_message},
};
use crate::{
    api::{
        ApiClient, ApiError, Secret,
        types::{DeviceRegistration, PublicJwk, UserProfile},
    },
    clock::Clock,
    kv::{KeyStoreKind, KeyValue, SecretStore},
};

/// At most one register per device per day.
pub const REGISTER_INTERVAL_MS: i64 = 24 * 60 * 60 * 1000;
const REGISTERED_PREFIX: &str = "easyquran.device.registered.";
/// Secret-store key prefix of an account's device key on this install.
pub const KEY_SECRET_PREFIX: &str = "eq.device.key.";

#[derive(Serialize, Deserialize)]
struct RegisteredRecord {
    at: i64,
    token: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct KeyRecord {
    device_id: String,
    /// Private scalar, base64url.
    d: Secret,
}

/// An account's device key on this install, paired with the id it signs
/// for. A record for another id counts as lost.
#[derive(Debug, Clone)]
pub struct StoredKey {
    pub device_id: String,
    pub key: DeviceKey,
}

fn key_secret_name(owner: i32) -> String {
    format!("{KEY_SECRET_PREFIX}{owner}")
}

/// The stored key of `owner`, if one is readable.
pub fn load_device_key(secrets: &dyn SecretStore, owner: i32) -> Option<StoredKey> {
    let raw = secrets.get(&key_secret_name(owner))?;
    let record: KeyRecord = serde_json::from_str(&raw).ok()?;
    let key = DeviceKey::from_secret(&record.d)?;
    Some(StoredKey {
        device_id: record.device_id,
        key,
    })
}

/// Keeps `stored` for `owner`; `false` when the store refused it.
pub fn save_device_key(secrets: &dyn SecretStore, owner: i32, stored: &StoredKey) -> bool {
    let record = KeyRecord {
        device_id: stored.device_id.clone(),
        d: stored.key.to_secret(),
    };
    let Ok(text) = serde_json::to_string(&record) else {
        return false;
    };
    secrets.set(&key_secret_name(owner), &text).is_ok()
}

/// Account deletion (F7): nothing on this install can sign for `owner`.
pub fn delete_device_key(secrets: &dyn SecretStore, owner: i32) {
    secrets.remove(&key_secret_name(owner));
}

fn registered_key(owner: i32) -> String {
    format!("{REGISTERED_PREFIX}{owner}")
}

/// Due when never registered, when the push token changed, or after 24 h.
pub fn registration_due(kv: &dyn KeyValue, owner: i32, token: Option<&str>, now_ms: i64) -> bool {
    let Some(last) = kv
        .get(&registered_key(owner))
        .and_then(|raw| serde_json::from_str::<RegisteredRecord>(&raw).ok())
    else {
        return true;
    };
    if token.is_some() && token != last.token.as_deref() {
        return true;
    }
    now_ms - last.at >= REGISTER_INTERVAL_MS
}

/// A removed push token must not look registered: the next call posts again.
pub fn forget_registered_token(kv: &dyn KeyValue, owner: i32) {
    let record = RegisteredRecord { at: 0, token: None };
    if let Ok(text) = serde_json::to_string(&record) {
        let _ = kv.set(&registered_key(owner), &text);
    }
}

/// What [`ensure_device_registered`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// Minor account (A34): never registers. Not a failure.
    Minor,
    /// Registered within the last day with the same token.
    NotDue,
    /// Registered; `device_id` is the id to keep. `keyed`: with a key proof.
    Registered { device_id: String, keyed: bool },
}

/// The key store kind this install claims, after the client label's rules.
pub fn claimed_key_store(env: &DeviceEnv, secrets: &dyn SecretStore) -> KeyStoreKind {
    key_store_label(env.platform, secrets.kind()).unwrap_or(KeyStoreKind::None)
}

struct Identity {
    device_id: String,
    key: Option<DeviceKey>,
}

/// The stored key for this id, or a fresh one. A failure means no key.
fn resolve_identity(
    kv: &dyn KeyValue,
    secrets: &dyn SecretStore,
    owner: i32,
    keyed: bool,
) -> Identity {
    let mut device_id = device_id_for(kv, owner, false);
    if !keyed {
        return Identity {
            device_id,
            key: None,
        };
    }
    if let Some(stored) = load_device_key(secrets, owner) {
        if stored.device_id == device_id {
            return Identity {
                device_id,
                key: Some(stored.key),
            };
        }
        // The key pairs with an id the server handed back earlier but the
        // id store lost: trust the key's pairing.
        if super::identity::valid_device_id(&stored.device_id) {
            set_device_id_for(kv, owner, &stored.device_id);
            device_id = stored.device_id;
            return Identity {
                device_id,
                key: Some(stored.key),
            };
        }
    }
    fresh_key(secrets, owner, device_id)
}

fn fresh_key(secrets: &dyn SecretStore, owner: i32, device_id: String) -> Identity {
    let stored = StoredKey {
        device_id,
        key: DeviceKey::generate(),
    };
    if save_device_key(secrets, owner, &stored) {
        return Identity {
            device_id: stored.device_id,
            key: Some(stored.key),
        };
    }
    Identity {
        device_id: stored.device_id,
        key: None,
    }
}

/// The keyless body; [`with_proof`] adds the key fields.
pub fn registration_body(
    env: &DeviceEnv,
    owner: i32,
    device_id: &str,
    token: Option<&str>,
) -> DeviceRegistration {
    DeviceRegistration {
        expected_user_id: owner,
        device_id: device_id.to_owned(),
        platform: env.platform.as_str().to_owned(),
        client: client_label(env.platform).to_owned(),
        form_factor: env.form_factor.as_str().to_owned(),
        browser: None,
        os_major: env.os_major.filter(|v| (1..=999).contains(v)),
        app_version: env
            .app_version
            .clone()
            .filter(|v| !v.is_empty() && v.chars().count() <= 32),
        token: token.map(str::to_owned),
        key_store: KeyStoreKind::None.as_str().to_owned(),
        public_key: None,
        nonce: None,
        signature: None,
        continuity_hint: None,
    }
}

/// Adds the key proof to `body`.
pub fn with_proof(
    mut body: DeviceRegistration,
    store: KeyStoreKind,
    public_key: PublicJwk,
    nonce: String,
    signature: String,
    hint: Option<String>,
) -> DeviceRegistration {
    body.key_store = store.as_str().to_owned();
    body.public_key = Some(public_key);
    body.nonce = Some(nonce);
    body.signature = Some(signature);
    body.continuity_hint = hint;
    body
}

async fn body_for(
    client: &ApiClient,
    env: &DeviceEnv,
    store: KeyStoreKind,
    owner: i32,
    token: Option<&str>,
    identity: &Identity,
) -> Result<DeviceRegistration, ApiError> {
    let base = registration_body(env, owner, &identity.device_id, token);
    let Some(key) = identity.key.as_ref() else {
        return Ok(base);
    };
    let challenge = client.device_challenge().await?;
    let signature = key.sign(&proof_message(&challenge.nonce, &identity.device_id, owner));
    let hint = match (env.platform.sends_hint(), env.os_id.as_deref()) {
        (true, Some(os_id)) if !os_id.trim().is_empty() => {
            b64u_decode(challenge.salt.expose()).map(|salt| continuity_hint(&salt, os_id))
        }
        _ => None,
    };
    Ok(with_proof(
        base,
        store,
        key.public_jwk(),
        challenge.nonce,
        signature,
        hint,
    ))
}

async fn attempt(
    client: &ApiClient,
    env: &DeviceEnv,
    store: KeyStoreKind,
    owner: i32,
    token: Option<&str>,
    identity: &Identity,
) -> Result<String, ApiError> {
    let body = body_for(client, env, store, owner, token, identity).await?;
    check_registration_shape(&body).map_err(|why| ApiError::Decode(why.to_owned()))?;
    let reply = client.device_register(&body).await?;
    Ok(reply.device_id)
}

/// Registers this install for `user` when due. The id the server keeps is
/// adopted, and the key re-paired with it.
pub async fn ensure_device_registered(
    client: &ApiClient,
    kv: &dyn KeyValue,
    clock: &dyn Clock,
    env: &DeviceEnv,
    user: &UserProfile,
    token: Option<&str>,
) -> Result<RegisterOutcome, ApiError> {
    if user.is_minor {
        super::identity::forget_device_id_for(kv, user.id);
        return Ok(RegisterOutcome::Minor);
    }
    let owner = user.id;
    if !registration_due(kv, owner, token, clock.unix_ms()) {
        return Ok(RegisterOutcome::NotDue);
    }
    let secrets = client.secrets().clone();
    let store = claimed_key_store(env, secrets.as_ref());
    let keyed = store != KeyStoreKind::None;

    let mut identity = resolve_identity(kv, secrets.as_ref(), owner, keyed);
    let mut result = attempt(client, env, store, owner, token, &identity).await;
    if matches!(&result, Err(ApiError::Http(e)) if e.status == 409) {
        let device_id = new_device_id();
        set_device_id_for(kv, owner, &device_id);
        identity = if keyed {
            fresh_key(secrets.as_ref(), owner, device_id)
        } else {
            Identity {
                device_id,
                key: None,
            }
        };
        result = attempt(client, env, store, owner, token, &identity).await;
    }
    let kept = result?;
    let kept = if super::identity::valid_device_id(&kept) {
        kept
    } else {
        identity.device_id.clone()
    };
    if kept != identity.device_id {
        set_device_id_for(kv, owner, &kept);
        if let Some(key) = identity.key.clone() {
            save_device_key(
                secrets.as_ref(),
                owner,
                &StoredKey {
                    device_id: kept.clone(),
                    key,
                },
            );
        }
    }
    let record = RegisteredRecord {
        at: clock.unix_ms(),
        token: token.map(str::to_owned),
    };
    if let Ok(text) = serde_json::to_string(&record) {
        let _ = kv.set(&registered_key(owner), &text);
    }
    Ok(RegisterOutcome::Registered {
        device_id: kept,
        keyed: identity.key.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::identity::{FormFactor, Platform};

    fn env(platform: Platform) -> DeviceEnv {
        DeviceEnv {
            platform,
            form_factor: FormFactor::Desktop,
            os_major: Some(15),
            app_version: Some("1.0.0".into()),
            os_id: Some("machine".into()),
        }
    }

    /// Every (client, platform, key_store) triple this app can send passes
    /// the backend's shape rules.
    #[test]
    fn every_triple_this_app_sends_passes_the_shape_check() {
        let platforms = [
            Platform::Ios,
            Platform::Android,
            Platform::Macos,
            Platform::Windows,
            Platform::Linux,
        ];
        let kinds = [
            KeyStoreKind::SecureHardware,
            KeyStoreKind::OsKeystore,
            KeyStoreKind::File,
            KeyStoreKind::None,
        ];
        let key = DeviceKey::generate();
        let nonce = super::super::keys::b64u_encode(&[7_u8; 32]);
        for platform in platforms {
            for kind in kinds {
                let e = env(platform);
                let store = key_store_label(platform, kind).unwrap_or(KeyStoreKind::None);
                let base = registration_body(&e, 7, "dev-1", None);
                let body = if store == KeyStoreKind::None {
                    base
                } else {
                    let hint = platform
                        .sends_hint()
                        .then(|| continuity_hint(&[1_u8; 32], "machine"));
                    with_proof(
                        base,
                        store,
                        key.public_jwk(),
                        nonce.clone(),
                        key.sign(&proof_message(&nonce, "dev-1", 7)),
                        hint,
                    )
                };
                assert!(
                    check_registration_shape(&body).is_ok(),
                    "{platform:?} {kind:?}: {:?}",
                    check_registration_shape(&body)
                );
            }
        }
    }

    #[test]
    fn due_rules_match_web() {
        let kv = crate::kv::MemoryKv::new();
        assert!(registration_due(&kv, 1, None, 0));
        let record = RegisteredRecord {
            at: 1_000,
            token: Some("t".into()),
        };
        kv.set(&registered_key(1), &serde_json::to_string(&record).unwrap())
            .unwrap();
        assert!(!registration_due(&kv, 1, None, 2_000));
        assert!(!registration_due(&kv, 1, Some("t"), 2_000));
        assert!(registration_due(&kv, 1, Some("u"), 2_000));
        assert!(registration_due(&kv, 1, None, 1_000 + REGISTER_INTERVAL_MS));
    }
}
