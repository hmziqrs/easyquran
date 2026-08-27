use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use sea_orm::{DatabaseTransaction, TransactionTrait};
use serde_json::json;
use tracing::instrument;

use crate::{
    db::sea_models::{bookmark, bookmark_folder},
    error::{DbResult, DbResultExt, ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::auth::AuthSession,
    AppState,
};

use super::validator::{MutationKind, SyncMutation, V1BookmarkSyncPayload};

/// Kind-specific requiredness the derive can't express on the flat wire
/// struct: bookmark.upsert needs surah + ayah, folder.upsert needs name.
fn ensure_kind_fields(m: &SyncMutation) -> Result<(), ErrorResponse> {
    let message = match m.kind {
        MutationKind::BookmarkUpsert if m.surah.is_none() || m.ayah.is_none() => {
            Some("surah and ayah are required for bookmark.upsert")
        }
        MutationKind::FolderUpsert if m.name.is_none() => {
            Some("name is required for folder.upsert")
        }
        _ => None,
    };
    match message {
        Some(msg) => Err(ErrorResponse::new(ErrorCode::InvalidInput).with_message(msg)),
        None => Ok(()),
    }
}

/// Resolve the wire folder_id against THIS user's folders: unknown or foreign
/// folder ids file the bookmark under root (folder_id = NULL) instead of
/// tripping the FK or leaking another user's folder.
async fn resolve_folder_id<'a>(
    tx: &DatabaseTransaction,
    user_id: i32,
    folder_id: Option<&'a str>,
) -> Result<Option<&'a str>, ErrorResponse> {
    match folder_id {
        Some(fid) if bookmark_folder::Entity::owned_by_user(tx, user_id, fid).await? => {
            Ok(Some(fid))
        }
        _ => Ok(None),
    }
}

/// Apply one mutation, LWW-resolved. Returns 1 when a row changed (applied),
/// 0 when skipped by LWW or the ownership guard.
async fn apply_mutation(
    tx: &DatabaseTransaction,
    user_id: i32,
    m: &SyncMutation,
) -> Result<u64, ErrorResponse> {
    match m.kind {
        MutationKind::FolderUpsert => {
            // ensure_kind_fields guaranteed Some(name) for folder.upsert.
            let name = m.name.as_deref().unwrap_or_default();
            bookmark_folder::Entity::upsert_lww(tx, user_id, &m.id, name, m.updated_at).await
        }
        MutationKind::FolderDelete => {
            bookmark_folder::Entity::delete_lww(tx, user_id, &m.id, m.updated_at).await
        }
        MutationKind::BookmarkUpsert => {
            let folder_id = resolve_folder_id(tx, user_id, m.folder_id.as_deref()).await?;
            let surah = m.surah.unwrap_or_default();
            let ayah = m.ayah.unwrap_or_default();
            bookmark::Entity::upsert_lww(tx, user_id, &m.id, folder_id, surah, ayah, m.updated_at)
                .await
        }
        MutationKind::BookmarkDelete => {
            bookmark::Entity::delete_lww(tx, user_id, &m.id, m.updated_at).await
        }
    }
}

/// One full round inside the caller's transaction: apply every mutation
/// (LWW-resolved), then read the snapshot from the SAME transaction — the
/// response can never observe a half-applied round.
async fn sync_round(
    tx: &DatabaseTransaction,
    user_id: i32,
    mutations: &[SyncMutation],
) -> DbResult<(
    u64,
    u64,
    Vec<bookmark_folder::FolderListItem>,
    Vec<bookmark::BookmarkListItem>,
)> {
    let mut applied: u64 = 0;
    let mut skipped: u64 = 0;
    for m in mutations {
        if apply_mutation(tx, user_id, m).await? > 0 {
            applied += 1;
        } else {
            skipped += 1;
        }
    }

    // Snapshot is unbounded by design: every round returns the user's ENTIRE
    // folder + bookmark set (full pull, server truth). Fine at bookmark scale
    // — rows are tiny and bounded by one account's history. If scale ever
    // demands it, the pagination seam is exactly these two list_for_user
    // reads (swap for a cursor variant); the LWW mutation core above stays
    // untouched.
    let folders: Vec<bookmark_folder::FolderListItem> =
        bookmark_folder::Entity::list_for_user(tx, user_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
    let bookmarks: Vec<bookmark::BookmarkListItem> = bookmark::Entity::list_for_user(tx, user_id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok((applied, skipped, folders, bookmarks))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn sync(
    state: State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1BookmarkSyncPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap(); // safe: route is behind auth_guard::verified
    let mutations = payload.mutations.as_deref().unwrap_or(&[]);

    // Validate every mutation before applying any: a bad item mid-queue must
    // reject the whole round, not leave a half-applied push.
    for m in mutations {
        ensure_kind_fields(m)?;
    }

    // Push and pull share ONE transaction: the resolve_folder_id
    // SELECT-then-insert sequence can no longer race a concurrent folder
    // delete into an FK violation (SQLite serializes writers inside the tx),
    // and any error rolls the whole round back — no partial state survives.
    let tx = state.sea_db.begin().await.map_err_to_response()?;
    let (applied, skipped, folders, bookmarks) = match sync_round(&tx, user.id, mutations).await {
        Ok(round) => round,
        Err(err) => {
            tx.rollback().await.map_err_to_response()?;
            return Err(err);
        }
    };
    tx.commit().await.map_err_to_response()?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "applied": applied,
            "skipped": skipped,
            "folders": folders,
            "bookmarks": bookmarks,
        })),
    ))
}
