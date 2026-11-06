use std::time::Duration;

use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use tracing::{error, info, instrument, warn};

use crate::db::sea_models::post::{ActiveModel, Column, Entity, PostStatus};
use crate::services::job_log::{self, OUTCOME_ERROR, OUTCOME_OK};
use crate::state::AppState;

const TICK_INTERVAL_SECS: u64 = 60;
const JOB_NAME: &str = "scheduled_post_publish";

const SYNC_SCRUB_INTERVAL_SECS: u64 = 3_600;
const SYNC_SCRUB_JOB: &str = "sync_tombstone_scrub";

pub fn start_scheduler(state: AppState) {
    tokio::spawn(run_sync_scrub(state.clone()));
    tokio::spawn(run(state));
    info!("Scheduled post publisher started (interval: {TICK_INTERVAL_SECS}s)");
}

async fn run_sync_scrub(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(SYNC_SCRUB_INTERVAL_SECS));
    loop {
        interval.tick().await;
        let started = job_log::epoch_now();
        let result = crate::services::sync_sweep::scrub_tombstones(&state.sea_db).await;
        let finished = job_log::epoch_now();
        let (outcome, detail) = match result {
            Ok(rows) => (OUTCOME_OK, Some(format!("scrubbed={rows}"))),
            Err(err) => {
                error!(error = %err, "Sync tombstone scrub failed");
                (OUTCOME_ERROR, None)
            }
        };
        job_log::record(
            &state.sea_db,
            SYNC_SCRUB_JOB,
            started,
            finished,
            outcome,
            detail.as_deref(),
        )
        .await;
    }
}

async fn run(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    loop {
        interval.tick().await;
        let started = job_log::epoch_now();
        let result = publish_due_posts(&state).await;
        let finished = job_log::epoch_now();
        match result {
            Ok((due, published)) => {
                let detail = format!("due={due} published={published}");
                job_log::record(
                    &state.sea_db,
                    JOB_NAME,
                    started,
                    finished,
                    OUTCOME_OK,
                    Some(&detail),
                )
                .await;
            }
            Err(err) => {
                error!(error = %err, "Scheduled post publisher tick failed");
                job_log::record(
                    &state.sea_db,
                    JOB_NAME,
                    started,
                    finished,
                    OUTCOME_ERROR,
                    None,
                )
                .await;
            }
        }
    }
}

/// Publishes due drafts. Returns `(due, published)`: due rows found this tick
/// and rows that actually flipped to published (skipped authors and failed
/// updates are the difference).
#[instrument(skip_all)]
async fn publish_due_posts(state: &AppState) -> Result<(usize, usize), sea_orm::DbErr> {
    let now = chrono::Utc::now().fixed_offset();

    let due_posts = Entity::find()
        .filter(Column::Status.eq(PostStatus::Draft))
        .filter(Column::PublishedAt.is_not_null())
        .filter(Column::PublishedAt.lte(now))
        .all(&state.sea_db)
        .await?;

    if due_posts.is_empty() {
        return Ok((0, 0));
    }

    let count = due_posts.len();
    let mut published = 0usize;
    for post in due_posts {
        let post_id = post.id;
        let author_id = post.author_id;

        // Fire-time re-authorization: the request-time check is a TOCTOU window — re-assert the Author role before publishing so a demoted/removed author isn't published by a background tick.
        let author_ok = match crate::db::sea_models::user::Entity::find_by_id(author_id)
            .one(&state.sea_db)
            .await
        {
            Ok(Some(u)) => u.is_author(),
            Ok(None) => false,
            Err(err) => {
                error!(
                    post_id, author_id, error = %err,
                    "Scheduler author re-check failed; skipping publish"
                );
                false
            }
        };
        if !author_ok {
            warn!(
                post_id,
                author_id, "Skipping scheduled publish: author no longer authorized"
            );
            continue;
        }

        let mut active: ActiveModel = post.into();
        active.status = Set(PostStatus::Published);
        if let Err(err) = active.update(&state.sea_db).await {
            error!(
                post_id,
                error = %err,
                "Failed to publish scheduled post"
            );
        } else {
            published += 1;
            info!(post_id, "Scheduled post published");
        }
    }

    info!(count, published, "Scheduled post publisher tick completed");
    Ok((count, published))
}
