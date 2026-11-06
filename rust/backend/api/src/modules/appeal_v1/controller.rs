use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use rand::Rng;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tracing::{info, instrument, warn};

use crate::db::sea_models::appeal;
use crate::db::sea_models::enforcement_notice;
use crate::error::{DbResult, ErrorCode, ErrorResponse};
use crate::utils::code_hash::hash_code;
use crate::AppState;

use super::dto::SubmitAppealPayload;

/// Hex length of an appeal token: 32 random bytes, 256 bits (floor is 128).
pub const TOKEN_HEX_LEN: usize = 64;

/// A fresh appeal token. Only its keyed HMAC is stored (`token_hash`).
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes[..]);
    hex::encode(bytes)
}

/// Keyed HMAC of an appeal token. Lookups go through this, never the raw token.
pub fn token_hash(secret_key: &[u8], token: &str) -> String {
    hash_code(secret_key, token)
}

/// The link in the notice mail. It is the only place the raw token leaves the server.
pub fn appeal_url(site_url: &str, token: &str) -> String {
    format!("{}/appeal/{token}", site_url.trim_end_matches('/'))
}

fn looks_like_token(token: &str) -> bool {
    token.len() == TOKEN_HEX_LEN && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Public appeal intake. Known, unknown and malformed tokens get the same 202 body
/// with the same headers, so the response is not a notice-existence oracle. An
/// invalid statement is refused before any token is looked at.
#[instrument(skip(state, payload))]
pub async fn submit(
    State(state): State<AppState>,
    Json(payload): Json<SubmitAppealPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let statement = payload.statement.trim();
    if statement.is_empty() || statement.chars().count() > appeal::MAX_STATEMENT_CHARS {
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("statement must be 1 to 2000 characters"));
    }
    if looks_like_token(&payload.token) {
        let hash = token_hash(&state.secret_key, &payload.token);
        if let Some(notice) =
            enforcement_notice::Entity::find_by_appeal_token_hash(&state.sea_db, &hash).await?
        {
            let recorded =
                appeal::Entity::submit(&state.sea_db, &notice, statement, Utc::now().timestamp())
                    .await?;
            info!(notice_id = notice.id, recorded, "appeal intake");
        }
    }
    Ok((StatusCode::ACCEPTED, Json(json!({ "received": true }))))
}

/// Decision-clock alert: open appeals past their due time. Logs a warning and
/// returns the count. Called on each admin ban-list load, since there is no
/// scheduler in this build.
pub async fn check_appeal_clock(conn: &DatabaseConnection, now: i64) -> DbResult<u64> {
    let overdue = appeal::Entity::overdue_count(conn, now).await?;
    if overdue > 0 {
        warn!(overdue, "open appeals past the decision clock");
    }
    Ok(overdue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_256_bit_hex_and_unique() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), TOKEN_HEX_LEN);
        assert!(looks_like_token(&a));
        assert_ne!(a, b);
    }

    #[test]
    fn stored_hash_is_keyed_and_never_the_token() {
        let token = new_token();
        let hash = token_hash(b"server-secret", &token);
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, token);
        assert_ne!(hash, token_hash(b"other-secret", &token));
    }

    #[test]
    fn appeal_url_has_one_slash_before_the_path() {
        assert_eq!(
            appeal_url("https://example.test/", "abc"),
            "https://example.test/appeal/abc"
        );
        assert_eq!(
            appeal_url("https://example.test", "abc"),
            "https://example.test/appeal/abc"
        );
    }

    #[test]
    fn only_64_hex_characters_are_looked_up() {
        assert!(!looks_like_token("not-a-token"));
        assert!(!looks_like_token(&"g".repeat(64)));
        assert!(looks_like_token(&"0".repeat(64)));
    }
}
