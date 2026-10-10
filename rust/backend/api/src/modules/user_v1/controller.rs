use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use axum_macros::debug_handler;
use sea_orm::{EntityTrait, TransactionError, TransactionTrait};
use serde_json::json;
use tracing::{error, info, instrument, warn};

use super::validator::*;
use crate::{
    config::settings::{web_auth, ProviderStatus},
    db::sea_models::{
        passkey_credential,
        user::{Entity as User, Model as UserModel, UserRole},
        user_oauth_identity,
    },
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::{
        account_methods::{can_unlink, total_sign_in_methods},
        auth::AuthSession,
    },
    AppState,
};

/// Providers a signed-in user may link/unlink from the account UI. Narrower than
/// `OAuthProvider`: only these three are currently offered as sign-in methods.
const SUPPORTED_IDENTITY_PROVIDERS: [&str; 3] = ["github", "discord", "google"];

fn is_supported_identity_provider(provider: &str) -> bool {
    SUPPORTED_IDENTITY_PROVIDERS.contains(&provider)
}

fn linkable_providers(configured: &[ProviderStatus], linked: &[&str]) -> Vec<String> {
    configured
        .iter()
        .filter(|status| {
            status.ready
                && is_supported_identity_provider(&status.name)
                && !linked.contains(&status.name.as_str())
        })
        .map(|status| status.name.clone())
        .collect()
}

async fn identity_snapshot(
    state: &AppState,
    user: &UserModel,
) -> Result<serde_json::Value, ErrorResponse> {
    let identities = user_oauth_identity::Entity::list_for_user(&state.sea_db, user.id).await?;
    let passkey_count = passkey_credential::Entity::count_for_user(&state.sea_db, user.id).await?;
    let has_password = user.password.is_some();
    let linked: Vec<&str> = identities
        .iter()
        .map(|identity| identity.provider.as_str())
        .collect();
    let views: Vec<serde_json::Value> = identities
        .iter()
        .map(|identity| {
            json!({
                "provider": identity.provider,
                "provider_user_id": identity.provider_user_id,
                "created_at": identity.created_at,
            })
        })
        .collect();

    Ok(json!({
        "identities": views,
        "has_password": has_password,
        "passkey_count": passkey_count,
        "can_unlink_any": can_unlink(total_sign_in_methods(
            identities.len(),
            has_password,
            passkey_count,
        )),
        "linkable_providers": linkable_providers(&web_auth().providers_status, &linked),
    }))
}

#[debug_handler]
#[instrument(skip(auth, state), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn list_identities(
    auth: AuthSession,
    state: State<AppState>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized)
            .with_message("You must be logged in to access this resource")
    })?;

    let body = identity_snapshot(&state, &user).await?;
    info!(user_id = user.id, "Sign-in identities listed");
    Ok((StatusCode::OK, Json(body)))
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn unlink_identity(
    auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1UnlinkIdentityPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized)
            .with_message("You must be logged in to access this resource")
    })?;
    let provider = payload.0.provider.trim().to_lowercase();
    if !is_supported_identity_provider(&provider) {
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Unsupported sign-in provider"));
    }

    let user_id = user.id;
    // Guard reads + delete in ONE transaction: separate pool checkouts let two
    // concurrent unlinks each pass the count guard and both delete, leaving the
    // account with zero sign-in methods (permanent lockout).
    let removed = state
        .sea_db
        .transaction::<_, u64, ErrorResponse>(|txn| {
            let provider = provider.clone();
            Box::pin(async move {
                // Re-read inside the txn: password presence can change concurrently.
                let Some(fresh_user) = User::find_by_id(user_id).one(txn).await? else {
                    return Err(ErrorResponse::new(ErrorCode::Unauthorized)
                        .with_message("You must be logged in to access this resource"));
                };

                let identities =
                    user_oauth_identity::Entity::list_for_user(txn, user_id).await?;
                if !identities
                    .iter()
                    .any(|identity| identity.provider == provider)
                {
                    return Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                        .with_message("This sign-in method is not linked to your account"));
                }

                let passkey_count =
                    passkey_credential::Entity::count_for_user(txn, user_id).await?;
                let total = total_sign_in_methods(
                    identities.len(),
                    fresh_user.password.is_some(),
                    passkey_count,
                );
                if !can_unlink(total) {
                    warn!(user_id, provider = %provider, "Refused to unlink the last sign-in method");
                    // 409 (BIZ_004): the request conflicts with the "keep one method" invariant.
                    return Err(ErrorResponse::new(ErrorCode::DependencyExists)
                        .with_message("Add a password or another sign-in method first"));
                }

                user_oauth_identity::Entity::delete_for_user_provider(txn, user_id, &provider).await
            })
        })
        .await
        .map_err(|err| match err {
            TransactionError::Connection(err) => ErrorResponse::from(err),
            TransactionError::Transaction(err) => err,
        })?;

    if removed == 0 {
        // Lost a race with a concurrent unlink; the method is already gone.
        return Err(ErrorResponse::new(ErrorCode::RecordNotFound)
            .with_message("This sign-in method is not linked to your account"));
    }

    info!(user_id = user.id, provider = %provider, "Unlinked OAuth sign-in method");
    let body = identity_snapshot(&state, &user).await?;
    Ok((StatusCode::OK, Json(body)))
}

#[debug_handler(state = AppState)]
#[instrument(skip(auth), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn get_profile(auth: AuthSession) -> Result<impl IntoResponse, ErrorResponse> {
    match auth.user {
        Some(user) => {
            info!(user_id = user.id, "Profile retrieved");
            Ok((StatusCode::OK, Json(json!(user))))
        }
        None => {
            warn!("Profile request with no authenticated user");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("No user with this ID exists"))
        }
    }
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn update_profile(
    auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1UpdateProfilePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized)
            .with_message("You must be logged in to access this resource")
    })?;

    let payload = payload.0.into_update_user();
    match User::update(&state.sea_db, user.id, payload).await {
        Ok(Some(user)) => {
            info!(user_id = user.id, "Profile updated");
            Ok((StatusCode::OK, Json(json!(user))))
        }
        Ok(None) => {
            warn!(user_id = user.id, "User not found during update");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("User could not be found or updated"))
        }
        Err(err) => {
            error!(user_id = user.id, "Failed to update profile: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(auth, state, payload))]
pub async fn admin_create(
    auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1AdminCreateUserPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Route guard only checks ROLE_ADMIN: without this a lower-rank admin could create a higher-rank user.
    let caller_level = auth.user.as_ref().map(|u| u.role.to_i32()).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let requested = UserRole::from_str(&payload.0.role)
        .map_err(|_| ErrorResponse::new(ErrorCode::InvalidInput).with_message("Invalid role"))?;
    if requested.to_i32() > caller_level {
        warn!(
            caller_level,
            requested = %payload.0.role,
            "Admin attempted to create a user above their own role"
        );
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("You cannot create a user with a role higher than your own"));
    }

    let payload = payload.0.into_new_user();
    let user = User::admin_create(&state.sea_db, &state.storage.config.public_url, payload).await?;
    info!(user_id = user.id, "Admin created user");
    Ok((StatusCode::CREATED, Json(json!(user))))
}

#[debug_handler]
#[instrument(skip(auth, state), fields(user_id))]
pub async fn admin_delete(
    auth: AuthSession,
    state: State<AppState>,
    Path(user_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Route guard only checks ROLE_ADMIN: block self-deletion and deletion of equal/higher-rank users (privilege escalation / self-lockout).
    let caller = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let caller_level = caller.role.to_i32();
    if caller.id == user_id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("You cannot delete your own account via the admin path"));
    }
    if let Some(target) = User::get_by_id(&state.sea_db, user_id).await? {
        if target.role.to_i32() >= caller_level {
            warn!(
                caller_level,
                target_level = target.role.to_i32(),
                "Admin attempted to delete an equal/higher-role user"
            );
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("You cannot delete a user at or above your own role"));
        }
    }
    match User::admin_delete(&state.sea_db, user_id).await {
        Ok(1) => {
            info!(user_id, "Admin deleted user");
            Ok((
                StatusCode::OK,
                Json(json!({ "message": "User deleted successfully" })),
            ))
        }
        Ok(0) => {
            warn!(user_id, "Admin tried to delete non-existent user");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("User does not exist"))
        }
        Ok(_) => {
            info!(user_id, "Admin deleted user");
            Ok((
                StatusCode::OK,
                Json(json!({ "message": "User deleted successfully" })),
            ))
        }
        Err(err) => {
            error!(user_id, "Failed to delete user: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id))]
pub async fn admin_update(
    auth: AuthSession,
    state: State<AppState>,
    Path(user_id): Path<i32>,
    payload: ValidatedJson<V1AdminUpdateUserPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Route guard only checks ROLE_ADMIN: requested role can't exceed the caller's, and the target must be strictly lower-rank.
    let caller_level = auth.user.as_ref().map(|u| u.role.to_i32()).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;

    if let Some(role_str) = payload.0.role.as_deref() {
        let requested = UserRole::from_str(role_str).map_err(|_| {
            ErrorResponse::new(ErrorCode::InvalidInput).with_message("Invalid role")
        })?;
        if requested.to_i32() > caller_level {
            warn!(
                caller_level,
                requested = role_str,
                "Admin attempted to assign a role above their own"
            );
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("You cannot assign a role higher than your own"));
        }
    }

    if let Some(target) = User::get_by_id(&state.sea_db, user_id).await? {
        if target.role.to_i32() >= caller_level {
            warn!(
                caller_level,
                target_level = target.role.to_i32(),
                "Admin attempted to modify an equal/higher-role user"
            );
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("You cannot modify a user at or above your own role"));
        }
    }

    let payload = payload.0.into_update_user();
    match User::admin_update(
        &state.sea_db,
        &state.storage.config.public_url,
        user_id,
        payload,
    )
    .await
    {
        Ok(Some(user)) => {
            info!(user_id, "Admin updated user");
            Ok((StatusCode::OK, Json(json!(user))))
        }
        Ok(None) => {
            warn!(user_id, "Admin tried to update non-existent user");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("No user with this ID exists"))
        }
        Err(err) => {
            error!(user_id, "Admin failed to update user: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id))]
pub async fn admin_change_password(
    auth: AuthSession,
    state: State<AppState>,
    Path(user_id): Path<i32>,
    payload: ValidatedJson<AdminChangePassword>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Route guard only checks ROLE_ADMIN: resetting a superior's password would be an account takeover.
    let caller_level = auth.user.as_ref().map(|u| u.role.to_i32()).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    if let Some(target) = User::get_by_id(&state.sea_db, user_id).await? {
        if target.role.to_i32() >= caller_level {
            warn!(
                caller_level,
                target_level = target.role.to_i32(),
                "Admin attempted to reset password of an equal/higher-role user"
            );
            return Err(
                ErrorResponse::new(ErrorCode::OperationNotAllowed).with_message(
                    "You cannot reset the password of a user at or above your own role",
                ),
            );
        }
    }
    User::change_password(&state.sea_db, user_id, payload.0.password).await?;
    info!(user_id, "Admin changed user password");
    Ok((
        StatusCode::OK,
        Json(json!({ "message": "Password changed successfully" })),
    ))
}

#[debug_handler]
#[instrument(skip(state, payload))]
pub async fn admin_list(
    state: State<AppState>,
    payload: ValidatedJson<V1AdminUserQueryParams>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let query = payload.0.into_user_query();
    let page = query.page.unwrap_or(1);

    let result = User::admin_list(&state.sea_db, &state.storage.config.public_url, query).await?;
    let users = result.data;
    let total = result.total;
    info!(total, page, "Admin listed users");
    Ok((
        StatusCode::OK,
        Json(json!({
            "data": users,
            "total": total,
            "per_page": User::PER_PAGE,
            "page": page,
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state), fields(user_id))]
pub async fn admin_view(
    state: State<AppState>,
    Path(user_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    match User::find_by_id_with_relations(&state.sea_db, &state.storage.config.public_url, user_id)
        .await
    {
        Ok(user) => {
            info!(user_id, "Admin viewed user");
            Ok((StatusCode::OK, Json(json!(user))))
        }
        Err(err) => {
            error!(user_id, "Admin failed to view user: {}", err);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_identity_providers_are_the_three_linkable_ones() {
        assert!(is_supported_identity_provider("github"));
        assert!(is_supported_identity_provider("discord"));
        assert!(is_supported_identity_provider("google"));
        assert!(!is_supported_identity_provider("facebook"));
        assert!(!is_supported_identity_provider("apple"));
        assert!(!is_supported_identity_provider(""));
    }

    #[test]
    fn linkable_providers_are_ready_supported_and_unlinked() {
        let configured = vec![
            ProviderStatus {
                name: "github".to_string(),
                ready: true,
            },
            ProviderStatus {
                name: "discord".to_string(),
                ready: false,
            },
            ProviderStatus {
                name: "google".to_string(),
                ready: true,
            },
            ProviderStatus {
                name: "facebook".to_string(),
                ready: true,
            },
            ProviderStatus {
                name: "apple".to_string(),
                ready: true,
            },
        ];

        let linkable = linkable_providers(&configured, &["github"]);
        assert_eq!(
            linkable,
            vec!["google"],
            "only ready, supported, not-yet-linked providers are offered"
        );
    }
}
