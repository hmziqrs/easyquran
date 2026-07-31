use crate::db::sea_models::app_constant::Entity as AppConstant;
use crate::db::sea_models::app_constant::Model as AppConstantModel;
use crate::error::{ErrorCode, ErrorResponse};
use crate::state::AppState;
use axum::extract::State;
use ruxlog_types::PaginatedList;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertConstantPayload {
    pub key: String,
    pub value: String,
    pub value_type: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub is_sensitive: bool,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstantsListParams {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub is_sensitive: Option<bool>,
    pub value_type: Option<String>,
}

pub struct AclService;

impl AclService {
    pub const VALUE_HASH: &'static str = "app_constants";
    pub const META_HASH: &'static str = "app_constants_meta";

    pub async fn bootstrap_from_env(
        State(state): State<AppState>,
    ) -> Result<serde_json::Value, ErrorResponse> {
        let env_vars: Vec<(String, String)> = std::env::vars().collect();
        for (key, value) in env_vars {
            let normalized_key = key.trim().to_string();
            if normalized_key.is_empty() {
                continue;
            }
            // Security: never persist secret env vars into app_constants at rest.
            if Self::looks_like_secret_key(&normalized_key) {
                tracing::debug!(
                    key = %normalized_key,
                    "Skipping secret env var during import_env (not persisted)"
                );
                continue;
            }
            let is_sensitive =
                Self::guess_sensitive(&normalized_key) || Self::guess_sensitive(&value);

            let _ = AppConstant::ensure_exists(
                &state.sea_db,
                &normalized_key,
                &value,
                None,
                is_sensitive,
                "env",
            )
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::InternalServerError).with_message(e.to_string())
            })?;
        }

        Self::sync_all_to_cache(State(state)).await?;

        Ok(json!({"message": "Env constants bootstrapped"}))
    }

    fn guess_sensitive(s: &str) -> bool {
        let lower = s.to_ascii_lowercase();
        lower.contains("secret")
            || lower.contains("password")
            || lower.contains("token")
            || lower.contains("key")
            || lower.contains("access")
    }

    fn looks_like_secret_key(key: &str) -> bool {
        let k = key.to_ascii_uppercase();
        const SECRET_NEEDLES: &[&str] = &[
            "SECRET",
            "PASSWORD",
            "PASSWD",
            "TOKEN",
            "CREDENTIAL",
            "PRIVATE",
            "SIGNING",
            "APIKEY",
            "DATABASE_URL",
            "DB_URL",
            "DSN",
            "PGCONN",
            "CONNECTION_STRING",
            "CONNECTIONSTRING",
            "REDIS_URL",
            "REDIS_TLS_URL",
            "SMTP_URL",
            "SMTP_PASS",
            "MAIL_PASSWORD",
            "AMQP",
            "RABBITMQ",
            "KAFKA",
            "COOKIE_KEY",
            "FIELD_ENC_KEY",
            "SESSION_KEY",
            "CSRF_KEY",
            "JWT_SECRET",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_ACCESS_KEY_ID",
            "AWS_SESSION_TOKEN",
            "STRIPE",
            "PADDLE",
            "LEMON",
            "RAZORPAY",
            "MERCADO",
            "GOOGLE_CLIENT_SECRET",
            "OAUTH_CLIENT_SECRET",
        ];
        const SECRET_SUFFIXES: &[&str] = &[
            "_KEY",
            "_SECRET",
            "_TOKEN",
            "_PASSWORD",
            "_PASSWD",
            "_PWD",
            "_CREDENTIAL",
            "_CREDENTIALS",
        ];
        SECRET_NEEDLES.iter().any(|n| k.contains(n))
            || SECRET_SUFFIXES.iter().any(|s| k.ends_with(s))
    }

    pub async fn get_constant(
        State(state): State<AppState>,
        key: &str,
    ) -> Result<AppConstantModel, ErrorResponse> {
        AppConstant::find_by_key(&state.sea_db, key)
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::InternalServerError).with_message(e.to_string())
            })?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Key not found")
            })
    }

    pub async fn list_constants(
        State(state): State<AppState>,
        params: ConstantsListParams,
    ) -> Result<PaginatedList<AppConstantModel>, ErrorResponse> {
        let page = params.page.unwrap_or(1);
        let per_page = params.per_page.unwrap_or(20);
        AppConstant::list(
            &state.sea_db,
            page,
            per_page,
            params.search,
            params.is_sensitive,
            params.value_type,
        )
        .await
        .map_err(|e| ErrorResponse::new(ErrorCode::InternalServerError).with_message(e.to_string()))
    }

    pub async fn upsert_constant(
        State(state): State<AppState>,
        payload: UpsertConstantPayload,
        updated_by: Option<i32>,
    ) -> Result<AppConstantModel, ErrorResponse> {
        let key = payload.key.trim().to_string();
        if key.is_empty() {
            return Err(
                ErrorResponse::new(ErrorCode::InvalidInput).with_message("Key cannot be empty")
            );
        }

        let value = payload.value;
        let value_type = payload.value_type.clone();
        let description = payload.description.clone();
        let is_sensitive = payload.is_sensitive;
        let source = payload.source.unwrap_or_else(|| "manual".to_string());

        let model = AppConstant::upsert_value(
            &state.sea_db,
            &key,
            &value,
            value_type.clone(),
            description,
            is_sensitive,
            &source,
            updated_by,
        )
        .await
        .map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError).with_message(e.to_string())
        })?;

        AppConstant::sync_all_to_cache(&state.sea_db, Self::VALUE_HASH, Self::META_HASH)
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::InternalServerError).with_message(e.to_string())
            })?;

        info!(key, "ACL constant upserted and cached");
        Ok(model)
    }

    pub async fn delete_constant(
        State(state): State<AppState>,
        key: String,
    ) -> Result<(), ErrorResponse> {
        AppConstant::delete_by_key(&state.sea_db, &key)
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::InternalServerError).with_message(e.to_string())
            })?;

        let _ =
            AppConstant::sync_all_to_cache(&state.sea_db, Self::VALUE_HASH, Self::META_HASH).await;

        Ok(())
    }

    pub async fn sync_all_to_cache(
        State(state): State<AppState>,
    ) -> Result<serde_json::Value, ErrorResponse> {
        AppConstant::sync_all_to_cache(&state.sea_db, Self::VALUE_HASH, Self::META_HASH)
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::InternalServerError).with_message(e.to_string())
            })?;

        Ok(json!({"message": "ACL cache synced"}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guess_sensitive_password() {
        assert!(AclService::guess_sensitive("DB_PASSWORD"));
        assert!(AclService::guess_sensitive("my-password-123"));
        assert!(AclService::guess_sensitive("userPassword"));
    }

    #[test]
    fn guess_sensitive_secret() {
        assert!(AclService::guess_sensitive("API_SECRET"));
        assert!(AclService::guess_sensitive("jwt-secret"));
        assert!(AclService::guess_sensitive("mySecretKey"));
    }

    #[test]
    fn guess_sensitive_token() {
        assert!(AclService::guess_sensitive("AUTH_TOKEN"));
        assert!(AclService::guess_sensitive("access-token"));
        assert!(AclService::guess_sensitive("refreshToken"));
    }

    #[test]
    fn guess_sensitive_key() {
        assert!(AclService::guess_sensitive("API_KEY"));
        assert!(AclService::guess_sensitive("encryption-key"));
        assert!(AclService::guess_sensitive("privateKey"));
    }

    #[test]
    fn guess_sensitive_access() {
        assert!(AclService::guess_sensitive("ACCESS_ID"));
        assert!(AclService::guess_sensitive("access_level"));
        assert!(AclService::guess_sensitive("rootAccess"));
    }

    #[test]
    fn guess_sensitive_normal_keys() {
        assert!(!AclService::guess_sensitive("DATABASE_URL"));
        assert!(!AclService::guess_sensitive("APP_NAME"));
        assert!(!AclService::guess_sensitive("PORT"));
        assert!(!AclService::guess_sensitive("HOST"));
        assert!(!AclService::guess_sensitive("DEBUG"));
        assert!(!AclService::guess_sensitive("LOG_LEVEL"));
    }

    #[test]
    fn guess_sensitive_case_insensitive() {
        assert!(AclService::guess_sensitive("Password"));
        assert!(AclService::guess_sensitive("SECRET"));
        assert!(AclService::guess_sensitive("Token"));
        assert!(AclService::guess_sensitive("KeY"));
        assert!(AclService::guess_sensitive("ACCESS"));
    }

    #[test]
    fn guess_sensitive_substring_match() {
        assert!(AclService::guess_sensitive("monkey"));
        assert!(AclService::guess_sensitive("inaccessibility"));
    }

    #[test]
    fn guess_sensitive_empty_string() {
        assert!(!AclService::guess_sensitive(""));
    }

    #[test]
    fn guess_sensitive_value_containing_keyword() {
        assert!(AclService::guess_sensitive("my_password_123"));
        assert!(AclService::guess_sensitive("bearer_token_value"));
    }
}
