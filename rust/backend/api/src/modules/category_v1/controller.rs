use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use axum_macros::debug_handler;
use serde_json::json;
use tracing::{error, info, instrument, warn};

use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

use crate::{
    db::sea_models::{category::Entity as Category, post},
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::auth::AuthSession,
    AppState,
};

use super::validator::{
    V1CategoryQueryParams, V1CreateCategoryPayload, V1DeleteCategoryPayload,
    V1UpdateCategoryPayload,
};

#[debug_handler]
#[instrument(skip(state, _auth, payload), fields(category_id))]
pub async fn create(
    State(state): State<AppState>,
    _auth: AuthSession,
    payload: ValidatedJson<V1CreateCategoryPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let new_category = payload.0.into_new_category();

    match Category::create(
        &state.sea_db,
        &state.storage.config.public_url,
        new_category,
    )
    .await
    {
        Ok(result) => {
            tracing::Span::current().record("category_id", result.id);
            info!(category_id = result.id, "Category created");
            Ok((StatusCode::CREATED, Json(json!(result))))
        }
        Err(err) => {
            error!("Failed to create category: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, _auth, payload), fields(category_id))]
pub async fn update(
    State(state): State<AppState>,
    _auth: AuthSession,
    Path(category_id): Path<i32>,
    payload: ValidatedJson<V1UpdateCategoryPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let update_category = payload.0.into_update_category();

    match Category::update(
        &state.sea_db,
        &state.storage.config.public_url,
        category_id,
        update_category,
    )
    .await
    {
        Ok(Some(category)) => {
            info!(category_id, "Category updated");
            Ok((StatusCode::OK, Json(json!(category))))
        }
        Ok(None) => {
            warn!(category_id, "Category not found for update");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("Category does not exist"))
        }
        Err(err) => {
            error!(category_id, "Failed to update category: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(category_id))]
pub async fn delete(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(category_id): Path<i32>,
    payload: ValidatedJson<V1DeleteCategoryPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let actor = auth.user_required()?;
    let Some(category) = Category::find_by_id(category_id)
        .one(&state.sea_db)
        .await
        .map_err(ErrorResponse::from)?
    else {
        warn!(category_id, "Category not found for delete");
        return Err(
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Category does not exist")
        );
    };
    // Deleting a category cascades every post in it: the caller must retype the slug.
    if payload.0.confirm_slug != category.slug {
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Confirmation does not match the category slug"));
    }
    let post_count = post::Entity::find()
        .filter(post::Column::CategoryId.eq(category_id))
        .count(&state.sea_db)
        .await
        .map_err(ErrorResponse::from)?;
    let snapshot = json!({
        "category": {
            "id": category.id,
            "name": category.name,
            "slug": category.slug,
            "parent_id": category.parent_id,
        },
        "post_count": post_count,
    });
    match Category::delete(&state.sea_db, category_id, actor.id, snapshot).await {
        Ok(0) => {
            warn!(category_id, "Category not found for delete");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("Category does not exist"))
        }
        Ok(_) => {
            info!(category_id, "Category deleted");
            Ok((
                StatusCode::OK,
                Json(json!({ "message": "Category deleted successfully" })),
            ))
        }
        Err(err) => {
            error!(category_id, "Failed to delete category: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state), fields(slug_or_id = %slug_or_id, category_id))]
pub async fn find_by_id_or_slug(
    State(state): State<AppState>,
    Path(slug_or_id): Path<String>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let mut id: Option<i32> = None;
    let mut slug: Option<String> = None;

    match slug_or_id.parse::<i32>() {
        Ok(parsed_id) => {
            id = Some(parsed_id);
        }
        Err(_) => {
            slug = Some(slug_or_id);
        }
    }

    match Category::find_by_id_or_slug(&state.sea_db, &state.storage.config.public_url, id, slug)
        .await
    {
        Ok(Some(category)) => {
            tracing::Span::current().record("category_id", category.id);
            info!(
                category_id = category.id,
                "Category retrieved by id or slug"
            );
            Ok((StatusCode::OK, Json(json!(category))))
        }
        Ok(None) => {
            warn!("Category not found");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Category not found"))
        }
        Err(err) => {
            error!("Failed to find category: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state))]
pub async fn find_all(State(state): State<AppState>) -> Result<impl IntoResponse, ErrorResponse> {
    match Category::find_all(&state.sea_db).await {
        Ok(categories) => {
            info!(count = categories.len(), "All categories retrieved");
            Ok((StatusCode::OK, Json(json!(categories))))
        }
        Err(err) => {
            error!("Failed to retrieve all categories: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, payload))]
pub async fn find_with_query(
    State(state): State<AppState>,
    payload: ValidatedJson<V1CategoryQueryParams>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let category_query = payload.0.into_category_query();
    let page = category_query.page.unwrap_or(1);

    match Category::find_with_query(
        &state.sea_db,
        &state.storage.config.public_url,
        category_query,
    )
    .await
    {
        Ok(result) => {
            info!(
                total = result.total,
                page, "Categories retrieved with query"
            );
            Ok((
                StatusCode::OK,
                Json(json!({
                    "data": result.data,
                    "total": result.total,
                    "per_page": Category::PER_PAGE,
                    "page": page,
                })),
            ))
        }
        Err(err) => {
            error!("Failed to query categories: {}", err);
            Err(err)
        }
    }
}
