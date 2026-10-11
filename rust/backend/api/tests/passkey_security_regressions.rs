use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use migration::{Migrator, MigratorTrait};
use openssl::{
    bn::{BigNum, BigNumContext},
    ec::{EcGroup, EcKey},
    hash::MessageDigest,
    nid::Nid,
    pkey::{PKey, Private},
    sign::Signer,
};
use ruxlog::db::sea_models::{passkey_credential, user};
use ruxlog::services::webauthn::WebauthnService;
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, Set};
use serde_json::json;
use sha2::{Digest, Sha256};
use webauthn_rs::prelude::{Passkey, PublicKeyCredential, RequestChallengeResponse, Uuid};

struct Authenticator {
    key: PKey<Private>,
    credential_id: Vec<u8>,
}
impl Authenticator {
    fn new() -> Self {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();
        Self {
            key: PKey::from_ec_key(key).unwrap(),
            credential_id: vec![0x6c; 32],
        }
    }
    fn passkey(&self) -> Passkey {
        let key = self.key.ec_key().unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let mut x = BigNum::new().unwrap();
        let mut y = BigNum::new().unwrap();
        key.public_key()
            .affine_coordinates_gfp(key.group(), &mut x, &mut y, &mut ctx)
            .unwrap();
        serde_json::from_value(json!({"cred": {
            "cred_id": URL_SAFE_NO_PAD.encode(&self.credential_id),
            "cred": { "type_": "ES256", "key": { "EC_EC2": {
                "curve": "SECP256R1", "x": URL_SAFE_NO_PAD.encode(x.to_vec_padded(32).unwrap()), "y": URL_SAFE_NO_PAD.encode(y.to_vec_padded(32).unwrap())
            }}},
            "counter": 0, "transports": ["internal"], "user_verified": true,
            "backup_eligible": false, "backup_state": false, "registration_policy": "required",
            "extensions": {}, "attestation": { "data": "None", "metadata": "None" }, "attestation_format": "none"
        }})).unwrap()
    }
    fn assertion(
        &self,
        challenge: &RequestChallengeResponse,
        user_id: i32,
        counter: u32,
        verified: bool,
    ) -> PublicKeyCredential {
        self.assertion_at(challenge, user_id, counter, verified, "https://example.com")
    }
    fn assertion_at(
        &self,
        challenge: &RequestChallengeResponse,
        user_id: i32,
        counter: u32,
        verified: bool,
        origin: &str,
    ) -> PublicKeyCredential {
        let client_data = serde_json::to_vec(&json!({ "type": "webauthn.get", "challenge": serde_json::to_value(challenge).unwrap()["publicKey"]["challenge"], "origin": origin, "crossOrigin": false })).unwrap();
        let mut authenticator_data = Sha256::digest(b"example.com").to_vec();
        authenticator_data.push(if verified { 0x05 } else { 0x01 });
        authenticator_data.extend(counter.to_be_bytes());
        let mut signed = authenticator_data.clone();
        signed.extend(Sha256::digest(&client_data));
        let mut signer = Signer::new(MessageDigest::sha256(), &self.key).unwrap();
        signer.update(&signed).unwrap();
        let signature = signer.sign_to_vec().unwrap();
        serde_json::from_value(json!({ "id": URL_SAFE_NO_PAD.encode(&self.credential_id), "rawId": URL_SAFE_NO_PAD.encode(&self.credential_id), "type": "public-key", "response": { "clientDataJSON": URL_SAFE_NO_PAD.encode(client_data), "authenticatorData": URL_SAFE_NO_PAD.encode(authenticator_data), "signature": URL_SAFE_NO_PAD.encode(signature), "userHandle": URL_SAFE_NO_PAD.encode(user_handle(user_id).as_bytes()) }, "extensions": {} })).unwrap()
    }
}
fn user_handle(id: i32) -> Uuid {
    let mut hash = Sha256::new();
    hash.update(b"ruxlog:user:");
    hash.update(id.to_be_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hash.finalize()[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
async fn fixture() -> (
    DatabaseConnection,
    WebauthnService,
    Authenticator,
    user::Model,
) {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let db = Database::connect(options).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    let now = chrono::Utc::now().fixed_offset();
    let owner = user::ActiveModel {
        name: Set("Passkey owner".into()),
        email: Set("passkey@example.com".into()),
        is_verified: Set(true),
        role: Set(user::UserRole::User),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let authenticator = Authenticator::new();
    passkey_credential::Entity::create(&db, owner.id, &authenticator.passkey(), None, None)
        .await
        .unwrap();
    let svc = WebauthnService::new("example.com", "https://example.com", "Test").unwrap();
    (db, svc, authenticator, owner)
}
#[tokio::test]
async fn signed_discoverable_assertion_logs_in_and_persists_counter() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 1, true);
    let (credential, logged_in) = svc.finish_login(&db, &assertion, &state).await.unwrap();
    assert_eq!(logged_in.id, owner.id);
    let stored = passkey_credential::Entity::find_by_id(credential.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.counter, 1);
    assert!(stored.last_used_at.is_some());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&stored.public_key).unwrap()["cred"]["counter"],
        1
    );
}
#[tokio::test]
async fn assertion_signed_by_wrong_key_is_rejected() {
    let (db, svc, _, owner) = fixture().await;
    let wrong = Authenticator::new();
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = wrong.assertion(&challenge, owner.id, 1, true);
    assert!(svc.finish_login(&db, &assertion, &state).await.is_err());
}
#[tokio::test]
async fn valid_signature_with_wrong_user_handle_is_rejected() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id + 1, 1, true);
    assert!(svc.finish_login(&db, &assertion, &state).await.is_err());
}
#[tokio::test]
async fn valid_signature_without_user_verification_is_rejected() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 1, false);
    assert!(svc.finish_login(&db, &assertion, &state).await.is_err());
}
#[tokio::test]
async fn valid_signature_from_wrong_origin_is_rejected() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let assertion =
        authenticator.assertion_at(&challenge, owner.id, 1, true, "https://evil.example");
    assert!(svc.finish_login(&db, &assertion, &state).await.is_err());
}
#[tokio::test]
async fn replayed_counter_is_rejected_even_with_new_signed_challenge() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 1, true);
    svc.finish_login(&db, &assertion, &state).await.unwrap();
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 1, true);
    assert!(svc.finish_login(&db, &assertion, &state).await.is_err());
}
#[tokio::test]
async fn deleted_credential_cannot_finish_in_flight_login() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 1, true);
    passkey_credential::Entity::delete_by_credential_id_for_user(
        &db,
        &URL_SAFE_NO_PAD.encode(&authenticator.credential_id),
        owner.id,
    )
    .await
    .unwrap();
    assert!(svc.finish_login(&db, &assertion, &state).await.is_err());
}

#[tokio::test]
async fn authenticators_without_counters_can_login_with_distinct_challenges() {
    let (db, svc, authenticator, owner) = fixture().await;
    for _ in 0..2 {
        let (challenge, state) = svc.start_login().unwrap();
        let assertion = authenticator.assertion(&challenge, owner.id, 0, true);
        assert!(svc.finish_login(&db, &assertion, &state).await.is_ok());
    }
}

#[tokio::test]
async fn established_counter_cannot_reset_to_zero() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 1, true);
    svc.finish_login(&db, &assertion, &state).await.unwrap();
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 0, true);
    assert!(svc.finish_login(&db, &assertion, &state).await.is_err());
}

#[tokio::test]
async fn stale_snapshot_cannot_overwrite_accepted_counter_or_deleted_key() {
    let (db, _, authenticator, owner) = fixture().await;
    let snapshot = passkey_credential::Entity::list_by_user(&db, owner.id)
        .await
        .unwrap()
        .remove(0);
    assert!(passkey_credential::Entity::touch_counter_if_unchanged(
        &db,
        &snapshot,
        &authenticator.passkey(),
        1
    )
    .await
    .unwrap());
    assert!(!passkey_credential::Entity::touch_counter_if_unchanged(
        &db,
        &snapshot,
        &authenticator.passkey(),
        1
    )
    .await
    .unwrap());
    let current = passkey_credential::Entity::list_by_user(&db, owner.id)
        .await
        .unwrap()
        .remove(0);
    passkey_credential::Entity::delete_by_credential_id_for_user(
        &db,
        &current.credential_id,
        owner.id,
    )
    .await
    .unwrap();
    assert!(!passkey_credential::Entity::touch_counter_if_unchanged(
        &db,
        &current,
        &authenticator.passkey(),
        2
    )
    .await
    .unwrap());
}

#[tokio::test]
async fn passkey_challenge_is_single_use_even_for_zero_counter_authenticator() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let handle = ruxlog::services::passkey_state::issue_authentication(&state, "session").unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 0, true);
    let state =
        ruxlog::services::passkey_state::consume_authentication(&handle, "session").unwrap();
    svc.finish_login(&db, &assertion, &state).await.unwrap();
    assert!(ruxlog::services::passkey_state::consume_authentication(&handle, "session").is_err());
}

async fn anonymous(db: &DatabaseConnection) -> ruxlog::services::auth::AuthSession {
    let store = std::sync::Arc::new(
        ruxlog::services::session_store::SqliteSessionStore::new(db.clone()).await,
    );
    let backend = ruxlog::services::auth::AuthBackend::new(
        db,
        store.clone(),
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
    );
    ruxlog::services::auth::AuthSession::new(
        backend,
        tower_sessions::Session::new(None, store, None),
    )
    .await
}

#[tokio::test]
async fn verified_passkey_still_requires_enrolled_totp_before_session() {
    let (db, svc, authenticator, owner) = fixture().await;
    ruxlog::utils::field_crypto::set_key(&[0x6b; 32]).unwrap();
    let mut updated: user::ActiveModel = owner.into();
    updated.two_fa_enabled = Set(true);
    updated.two_fa_secret = Set(Some("JBSWY3DPEHPK3PXP".into()));
    let owner = updated.update(&db).await.unwrap();
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 1, true);
    let (credential, owner) = svc.finish_login(&db, &assertion, &state).await.unwrap();
    let mut auth = anonymous(&db).await;
    let (_, _, axum::Json(body)) = ruxlog::modules::passkey_v1::controller::complete_passkey_login(
        &db,
        &mut auth,
        &credential,
        &owner,
    )
    .await
    .unwrap();
    assert_eq!(body["status"], "totp_required");
    assert!(body.get("user").is_none());
    assert!(auth.user.is_none());
    assert!(ruxlog::services::pending_login::response(&auth)
        .await
        .unwrap()
        .is_some());
    use sea_orm::PaginatorTrait;
    assert_eq!(
        ruxlog::db::sea_models::user_session::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn verified_passkey_without_totp_gets_bound_session_and_fresh_grant() {
    let (db, svc, authenticator, owner) = fixture().await;
    let (challenge, state) = svc.start_login().unwrap();
    let assertion = authenticator.assertion(&challenge, owner.id, 1, true);
    let (credential, owner) = svc.finish_login(&db, &assertion, &state).await.unwrap();
    let mut auth = anonymous(&db).await;
    let (_, _, axum::Json(body)) = ruxlog::modules::passkey_v1::controller::complete_passkey_login(
        &db,
        &mut auth,
        &credential,
        &owner,
    )
    .await
    .unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(auth.user.as_ref().unwrap().id, owner.id);
    use sea_orm::PaginatorTrait;
    assert_eq!(
        ruxlog::db::sea_models::user_session::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        1
    );
    ruxlog::services::fresh_auth::consume(&auth, &db)
        .await
        .unwrap();
    assert!(ruxlog::services::fresh_auth::consume(&auth, &db)
        .await
        .is_err());
}
