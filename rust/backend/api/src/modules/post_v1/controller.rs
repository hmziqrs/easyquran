use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use axum_client_ip::ClientIp;

use crate::db::sea_models::post::UpdatePost;
use crate::db::sea_models::{post_revision, post_series, post_series_post, scheduled_post};
use axum_macros::debug_handler;
use sea_orm::EntityTrait;
use serde_json::json;
use std::collections::HashSet;
use tracing::{error, info, instrument, warn};

use crate::db::sea_models::user::{self, UserRole};
use crate::{
    db::sea_models::post,
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    modules::post_v1::validator::V1UpdatePostPayload,
    services::{
        auth::AuthSession,
        paywall::{self, PostAccessPolicy},
    },
    AppState,
};

use super::validator::{
    V1AutosavePayload, V1CreatePostPayload, V1PostQueryParams, V1SchedulePayload,
    V1SeriesCreatePayload, V1SeriesListQuery, V1SeriesUpdatePayload,
};

fn viewer_bypasses_paywall(viewer: Option<&user::Model>, author_id: i32) -> bool {
    match viewer {
        Some(u) => u.is_moderator() || u.id == author_id,
        None => false,
    }
}

fn can_mutate_post(viewer: &user::Model, author_id: i32) -> bool {
    (viewer.id == author_id && viewer.is_author()) || viewer.is_admin()
}

async fn require_post_ownership(
    state: &AppState,
    post_id: i32,
    viewer: &user::Model,
) -> Result<(), ErrorResponse> {
    let post = post::Entity::find_by_id(post_id)
        .one(&state.sea_db)
        .await
        .map_err(|err| {
            error!(error = ?err, post_id, "DB error while checking post ownership");
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Failed to verify post ownership")
        })?;

    match post {
        Some(model) if can_mutate_post(viewer, model.author_id) => Ok(()),
        Some(_) => {
            warn!(
                user_id = viewer.id,
                post_id, "Ownership check failed for post mutation"
            );
            Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("You do not have permission to modify this post"))
        }
        None => {
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Post does not exist"))
        }
    }
}

async fn apply_paywall_single(
    state: &AppState,
    post: &mut post::PostWithRelations,
    viewer: Option<&user::Model>,
) -> Result<(), ErrorResponse> {
    let bypass = viewer_bypasses_paywall(viewer, post.author.id);
    let outcome =
        paywall::user_has_access(&state.sea_db, viewer.map(|u| u.id), post.id, bypass).await?;
    post.access_type = outcome.policy.access_type;
    post.price_cents = outcome.policy.price_cents;
    post.currency = outcome.policy.currency.clone();
    post.has_access = outcome.granted;
    if !outcome.granted {
        post.content = serde_json::Value::Object(serde_json::Map::new());
    }
    Ok(())
}

async fn apply_paywall_list(
    state: &AppState,
    posts: &mut [post::PostWithRelations],
    viewer: Option<&user::Model>,
) -> Result<(), ErrorResponse> {
    let ids: Vec<i32> = posts.iter().map(|p| p.id).collect();
    let policies = paywall::load_post_access_map(&state.sea_db, &ids).await?;

    let is_staff = viewer.map(|u| u.is_moderator()).unwrap_or(false);
    let (purchased, has_active_sub) = if let Some(u) = viewer.filter(|_| !is_staff) {
        (
            paywall::user_purchased_post_ids(&state.sea_db, u.id, &ids).await?,
            paywall::user_has_active_subscription(&state.sea_db, u.id).await?,
        )
    } else {
        (HashSet::new(), false)
    };

    for post in posts {
        let policy = policies
            .get(&post.id)
            .cloned()
            .unwrap_or_else(PostAccessPolicy::free);
        let bypass = is_staff || viewer.map(|u| u.id == post.author.id).unwrap_or(false);
        let granted = paywall::decide_access(
            &policy,
            bypass,
            purchased.contains(&post.id),
            has_active_sub,
        );
        post.access_type = policy.access_type;
        post.price_cents = policy.price_cents;
        post.currency = policy.currency.clone();
        post.has_access = granted;
        if !granted {
            post.content = serde_json::Value::Object(serde_json::Map::new());
        }
    }
    Ok(())
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, post_id, slug, result))]
pub async fn create(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1CreatePostPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    tracing::Span::current().record("user_id", user.id);

    info!(user_id = user.id, "Creating post");

    let new_post = payload.0.into_new_post(user.id);

    match post::Entity::create(&state.sea_db, &state.storage.config.public_url, new_post).await {
        Ok(post) => {
            info!(post_id = post.id, slug = %post.slug, "Post created successfully");
            tracing::Span::current().record("post_id", post.id);
            tracing::Span::current().record("slug", &post.slug);
            tracing::Span::current().record("result", "success");
            Ok((StatusCode::CREATED, Json(json!(post))))
        }
        Err(err) => {
            error!(error = ?err, user_id = user.id, "Failed to create post");
            tracing::Span::current().record("result", "failure");
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(identifier = %slug_or_id, post_id, result))]
pub async fn find_by_id_or_slug(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(slug_or_id): Path<String>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!(identifier = %slug_or_id, "Finding post by ID or slug");

    let query = match slug_or_id.parse::<i32>() {
        Ok(id) => {
            info!(post_id = id, "Searching by ID");
            post::Entity::find_by_id_or_slug(
                &state.sea_db,
                &state.storage.config.public_url,
                Some(id),
                None,
            )
            .await
        }
        Err(_) => {
            info!(slug = %slug_or_id, "Searching by slug");
            post::Entity::find_by_id_or_slug(
                &state.sea_db,
                &state.storage.config.public_url,
                None,
                Some(slug_or_id),
            )
            .await
        }
    };

    match query {
        Ok(Some(mut post)) => {
            info!(post_id = post.id, "Post found");
            tracing::Span::current().record("post_id", post.id);
            tracing::Span::current().record("result", "found");

            let bypass = viewer_bypasses_paywall(auth.user.as_ref(), post.author.id);
            if !bypass && post.status != post::PostStatus::Published {
                tracing::Span::current().record("result", "hidden_status");
                return Err(
                    ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Post not found")
                );
            }

            apply_paywall_single(&state, &mut post, auth.user.as_ref()).await?;
            Ok((StatusCode::OK, Json(json!(post))))
        }
        Ok(None) => {
            warn!("Post not found");
            tracing::Span::current().record("result", "not_found");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Post not found"))
        }
        Err(err) => {
            error!(error = ?err, "Database error while finding post");
            tracing::Span::current().record("result", "error");
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(post_id = %post_id, result))]
pub async fn update(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(post_id): Path<i32>,
    payload: ValidatedJson<V1UpdatePostPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!(post_id, "Updating post");

    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    require_post_ownership(&state, post_id, &user).await?;

    let update_post = payload.0.into_update_post();

    match post::Entity::update(
        &state.sea_db,
        &state.storage.config.public_url,
        post_id,
        update_post,
    )
    .await
    {
        Ok(Some(post)) => {
            info!(post_id, slug = %post.slug, "Post updated successfully");
            tracing::Span::current().record("result", "success");
            Ok((StatusCode::OK, Json(json!(post))))
        }
        Ok(None) => {
            warn!(post_id, "Post not found for update");
            tracing::Span::current().record("result", "not_found");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Post does not exist"))
        }
        Err(err) => {
            error!(error = ?err, post_id, "Failed to update post");
            tracing::Span::current().record("result", "failure");
            Err(err)
        }
    }
}

#[debug_handler]
pub async fn delete(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(post_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    require_post_ownership(&state, post_id, &user).await?;

    match post::Entity::delete(&state.sea_db, post_id).await {
        Ok(1) => Ok((
            StatusCode::OK,
            Json(json!({ "message": "Post deleted successfully" })),
        )),
        Ok(0) => {
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Post does not exist"))
        }
        Ok(_) => Err(ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Internal server error occurred while deleting post")),
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn find_published_posts(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1PostQueryParams>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let page = payload.page.unwrap_or(1);
    match post::Entity::find_published_paginated(
        &state.sea_db,
        &state.storage.config.public_url,
        payload.0.into_post_query(),
    )
    .await
    {
        Ok(result) => {
            let mut posts = result.data;
            let total = result.total;
            apply_paywall_list(&state, &mut posts, auth.user.as_ref()).await?;
            Ok((
                StatusCode::OK,
                Json(json!({
                    "data": posts,
                    "total": total,
                    "per_page": post::Entity::PER_PAGE,
                    "page": page,
                })),
            ))
        }
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn track_view(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(post_id): Path<i32>,
    client_ip: ClientIp,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user_id: Option<i32> = auth.user.map(|user| user.id);

    // Dedup per (post, ip): rate-limiting caps requests/IP, but an attacker rotating
    // IPs still yields one DB write per request. This bounds writes to one per
    // (post, ip) per 5 min. Fail-open on store error.
    let ip_str = client_ip.0.to_string();
    let dedup_key = format!("trackview:{post_id}:{ip_str}");
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    if !rux_request_gate::dedup_nx(&state.gate_store, &dedup_key, 300).await {

        return Ok((
            StatusCode::OK,
            Json(json!({ "message": "View tracked successfully" })),
        ));
    }

    match post::Entity::increment_view_count(
        &state.sea_db,
        post_id,
        user_id,
        Some(ip_str),
        user_agent,
    )
    .await
    {
        Ok(_) => Ok((
            StatusCode::OK,
            Json(json!({ "message": "View tracked successfully" })),
        )),
        Err(err) => Err(ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to track view")
            .with_details(err.to_string())),
    }
}

#[debug_handler]
pub async fn sitemap(State(state): State<AppState>) -> Result<impl IntoResponse, ErrorResponse> {
    match post::Entity::sitemap(&state.sea_db).await {
        Ok(posts) => Ok((StatusCode::OK, Json(posts))),
        Err(err) => Err(ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to fetch posts")
            .with_details(err.to_string())),
    }
}

#[debug_handler]
pub async fn query(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1PostQueryParams>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let mut query_params = payload.0.clone();

    match user.role {
        UserRole::Author => {
            query_params.author_id = Some(user.id);
        }
        UserRole::Admin | UserRole::SuperAdmin => {}
        UserRole::Moderator => {
        }
        UserRole::User => {
            return Err(
                ErrorResponse::new(ErrorCode::OperationNotAllowed).with_message("Access denied")
            );
        }
    }

    let page = query_params.page.unwrap_or(1);

    match post::Entity::search(
        &state.sea_db,
        &state.storage.config.public_url,
        query_params.into_post_query(),
    )
    .await
    {
        Ok(result) => {
            let mut posts = result.data;
            let total = result.total;
            apply_paywall_list(&state, &mut posts, Some(&user)).await?;
            Ok((
                StatusCode::OK,
                Json(json!({
                    "data": posts,
                    "total": total,
                    "per_page": post::Entity::PER_PAGE,
                    "page": page,
                })),
            ))
        }
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn autosave(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1AutosavePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let p = payload.0;

    require_post_ownership(&state, p.post_id, &user).await?;

    match post_revision::Entity::create(
        &state.sea_db,
        p.post_id,
        serde_json::to_string(&p.content).unwrap_or_else(|_| "{}".to_string()),
        None,
    )
    .await
    {
        Ok(revision) => {
            let update = UpdatePost {
                title: None,
                slug: None,
                content: Some(serde_json::to_value(&p.content).unwrap_or(serde_json::json!({}))),
                excerpt: None,
                featured_image_id: None,
                status: None,
                published_at: None,
                updated_at: p.updated_at,
                category_id: None,
                view_count: None,
                likes_count: None,
                tag_ids: None,
            };

            match post::Entity::update(
                &state.sea_db,
                &state.storage.config.public_url,
                p.post_id,
                update,
            )
            .await
            {
                Ok(_) => Ok((StatusCode::OK, Json(json!(revision)))),
                Err(err) => Err(err),
            }
        }
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn revisions_list(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(post_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;

    require_post_ownership(&state, post_id, &user).await?;

    let page: u64 = 1;

    match post_revision::Entity::list_by_post(&state.sea_db, post_id, Some(page), None).await {
        Ok(result) => Ok((
            StatusCode::OK,
            Json(json!({ "data": result.data, "total": result.total, "page": page })),
        )),
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn revisions_restore(
    State(state): State<AppState>,
    auth: AuthSession,
    Path((post_id, revision_id)): Path<(i32, i32)>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;

    require_post_ownership(&state, post_id, &user).await?;

    let rev_opt = match post_revision::Entity::find_by_id(revision_id)
        .one(&state.sea_db)
        .await
    {
        Ok(r) => r,
        Err(err) => return Err(err.into()),
    };

    let rev = if let Some(r) = rev_opt {
        r
    } else {
        return Err(
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Revision not found")
        );
    };

    if rev.post_id != post_id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Revision does not belong to the specified post"));
    }

    let now = chrono::Utc::now().fixed_offset();
    let update = UpdatePost {
        title: None,
        slug: None,
        content: Some(serde_json::from_str(&rev.content).unwrap_or(serde_json::json!({}))),
        excerpt: None,
        featured_image_id: None,
        status: None,
        published_at: None,
        updated_at: now,
        category_id: None,
        view_count: None,
        likes_count: None,
        tag_ids: None,
    };

    match post::Entity::update(
        &state.sea_db,
        &state.storage.config.public_url,
        post_id,
        update,
    )
    .await
    {
        Ok(_) => {
            let meta = serde_json::json!({ "restored_from_revision_id": revision_id });
            match post_revision::Entity::create(
                &state.sea_db,
                post_id,
                rev.content.clone(),
                Some(meta),
            )
            .await
            {
                Ok(new_rev) => Ok((StatusCode::OK, Json(json!(new_rev)))),
                Err(err) => Err(err),
            }
        }
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn schedule(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1SchedulePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let p = payload.0;

    require_post_ownership(&state, p.post_id, &user).await?;

    match scheduled_post::Entity::upsert(&state.sea_db, p.post_id, p.publish_at).await {
        Ok(model) => Ok((StatusCode::OK, Json(json!(model)))),
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn series_create(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1SeriesCreatePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    if !user.is_admin() {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Only staff may manage post series"));
    }
    let p = payload.0;

    match post_series::Entity::create(&state.sea_db, p.name, p.slug, p.description).await {
        Ok(series) => Ok((StatusCode::CREATED, Json(json!(series)))),
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn series_update(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(series_id): Path<i32>,
    payload: ValidatedJson<V1SeriesUpdatePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    if !user.is_admin() {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Only staff may manage post series"));
    }

    match post_series::Entity::update(
        &state.sea_db,
        series_id,
        payload.0.name,
        payload.0.slug,
        payload.0.description,
    )
    .await
    {
        Ok(Some(series)) => Ok((StatusCode::OK, Json(json!(series)))),
        Ok(None) => {
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Series not found"))
        }
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn series_delete(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(series_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    if !user.is_admin() {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Only staff may manage post series"));
    }

    match post_series::Entity::delete(&state.sea_db, series_id).await {
        Ok(1) => Ok((
            StatusCode::OK,
            Json(json!({ "message": "Deleted successfully" })),
        )),
        Ok(0) => {
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Series not found"))
        }
        Ok(_) => Err(ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Internal server error occurred while deleting series")),
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn series_list(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1SeriesListQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let _user = auth.user.unwrap();
    let page = payload.page.unwrap_or(1);

    match post_series::Entity::list(&state.sea_db, payload.page, None, payload.search.clone()).await
    {
        Ok(result) => {
            let total = result.total;
            let items = result.data;
            let mut data = Vec::with_capacity(items.len());
            for s in items {
                let count = post_series_post::Entity::count_by_series(&state.sea_db, s.id)
                    .await
                    .unwrap_or(0);
                data.push(serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "slug": s.slug,
                    "description": s.description,
                    "created_at": s.created_at,
                    "updated_at": s.updated_at,
                    "posts_count": count as i64,
                }));
            }

            Ok((
                StatusCode::OK,
                Json(json!({ "data": data, "total": total, "page": page })),
            ))
        }
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn series_add(
    State(state): State<AppState>,
    auth: AuthSession,
    Path((post_id, series_id)): Path<(i32, i32)>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;

    require_post_ownership(&state, post_id, &user).await?;

    let payload = post_series_post::NewPostSeriesPost {
        series_id,
        post_id,
        sort_order: None,
    };

    match post_series_post::Entity::add(&state.sea_db, payload).await {
        Ok(model) => Ok((StatusCode::CREATED, Json(json!(model)))),
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn series_remove(
    State(state): State<AppState>,
    auth: AuthSession,
    Path((post_id, series_id)): Path<(i32, i32)>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;

    require_post_ownership(&state, post_id, &user).await?;

    let payload = post_series_post::RemovePostSeriesPost { series_id, post_id };

    match post_series_post::Entity::remove(&state.sea_db, payload).await {
        Ok(affected) if affected > 0 => Ok((
            StatusCode::OK,
            Json(json!({ "message": "Removed successfully" })),
        )),
        Ok(_) => {
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Mapping not found"))
        }
        Err(err) => Err(err),
    }
}

use crate::db::sea_models::post_like;

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id, post_id))]
pub async fn like_post(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(post_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    tracing::Span::current().record("user_id", user.id);
    tracing::Span::current().record("post_id", post_id);

    match post_like::Entity::like_post(&state.sea_db, post_id, user.id).await {
        Ok((true, likes_count)) => {
            info!(user_id = user.id, post_id, likes_count, "Post liked");
            Ok((
                StatusCode::OK,
                Json(json!(post_like::LikeActionResponse {
                    post_id,
                    is_liked: true,
                    likes_count,
                    message: "Post liked successfully".to_string(),
                })),
            ))
        }
        Ok((false, likes_count)) => {
            warn!(user_id = user.id, post_id, "Post already liked");
            Ok((
                StatusCode::OK,
                Json(json!(post_like::LikeActionResponse {
                    post_id,
                    is_liked: true,
                    likes_count,
                    message: "Post was already liked".to_string(),
                })),
            ))
        }
        Err(err) => {
            error!(user_id = user.id, post_id, "Failed to like post: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id, post_id))]
pub async fn unlike_post(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(post_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    tracing::Span::current().record("user_id", user.id);
    tracing::Span::current().record("post_id", post_id);

    match post_like::Entity::unlike_post(&state.sea_db, post_id, user.id).await {
        Ok((true, likes_count)) => {
            info!(user_id = user.id, post_id, likes_count, "Post unliked");
            Ok((
                StatusCode::OK,
                Json(json!(post_like::LikeActionResponse {
                    post_id,
                    is_liked: false,
                    likes_count,
                    message: "Post unliked successfully".to_string(),
                })),
            ))
        }
        Ok((false, likes_count)) => {
            warn!(user_id = user.id, post_id, "Post was not liked");
            Ok((
                StatusCode::OK,
                Json(json!(post_like::LikeActionResponse {
                    post_id,
                    is_liked: false,
                    likes_count,
                    message: "Post was not liked".to_string(),
                })),
            ))
        }
        Err(err) => {
            error!(user_id = user.id, post_id, "Failed to unlike post: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id, post_id))]
pub async fn like_status(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(post_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    tracing::Span::current().record("user_id", user.id);
    tracing::Span::current().record("post_id", post_id);

    match post_like::Entity::get_like_status(&state.sea_db, post_id, user.id).await {
        Ok(status) => Ok((StatusCode::OK, Json(json!(status)))),
        Err(err) => {
            error!(
                user_id = user.id,
                post_id, "Failed to get like status: {}", err
            );
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, post_count))]
pub async fn like_status_batch(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<post_like::LikeStatusBatchRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    tracing::Span::current().record("user_id", user.id);
    tracing::Span::current().record("post_count", payload.post_ids.len());

    match post_like::Entity::get_like_status_batch(&state.sea_db, &payload.post_ids, user.id).await
    {
        Ok(statuses) => Ok((
            StatusCode::OK,
            Json(json!(post_like::LikeStatusBatchResponse { statuses })),
        )),
        Err(err) => {
            error!(
                user_id = user.id,
                "Failed to get batch like status: {}", err
            );
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_user(id: i32, role: UserRole) -> user::Model {
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .unwrap()
            .fixed_offset();
        user::Model {
            id,
            name: format!("user-{id}"),
            email: format!("user-{id}@example.com"),
            password: None,
            avatar_id: None,
            is_verified: true,
            role,
            two_fa_enabled: false,
            two_fa_secret: None,
            two_fa_backup_codes: None,
            two_fa_last_totp_counter: None,
            google_id: None,
            oauth_provider: None,
            session_auth_secret: format!("test-secret-{id}"),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn author_can_mutate_own_post() {
        let author = make_user(7, UserRole::Author);
        assert!(can_mutate_post(&author, 7));
    }

    #[test]
    fn other_author_cannot_mutate_foreign_post() {
        let author_a = make_user(7, UserRole::Author);
        let author_b = make_user(8, UserRole::Author);
        assert!(!can_mutate_post(&author_a, 8));
        assert!(!can_mutate_post(&author_b, 7));
    }

    #[test]
    fn staff_role_bypasses_ownership() {
        let admin = make_user(100, UserRole::Admin);
        let super_admin = make_user(101, UserRole::SuperAdmin);
        assert!(can_mutate_post(&admin, 7));
        assert!(can_mutate_post(&super_admin, 7));
    }

    #[test]
    fn plain_user_role_is_denied() {
        let plain = make_user(7, UserRole::User);
        assert!(!can_mutate_post(&plain, 7));
    }

    #[test]
    fn moderator_does_not_bypass_on_writes() {
        let mod_user = make_user(200, UserRole::Moderator);
        assert!(!can_mutate_post(&mod_user, 7));
    }
}
