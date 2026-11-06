use crate::error::{DbResult, ErrorCode, ErrorResponse};
use ruxlog_types::PaginatedList;
use sea_orm::{entity::prelude::*, Condition, Order, QueryOrder, QuerySelect, Set};
use tracing::{error, info, instrument, warn};

use super::{
    ActiveModel, Column, Entity, Model, NewSubscriber, SubscriberListItem, SubscriberQuery,
    SubscriberStatus, UpdateSubscriber,
};

const DAY_SECS: i64 = 24 * 60 * 60;

/// Retention for a subscriber row in each status (A29): confirmed 12 months,
/// pending and unsubscribed 30 days. Returns (expires_at epoch secs, class).
pub fn retention_for(status: SubscriberStatus, now: i64) -> (i64, &'static str) {
    match status {
        SubscriberStatus::Confirmed => (now + 365 * DAY_SECS, "newsletter_confirmed"),
        SubscriberStatus::Unsubscribed => (now + 30 * DAY_SECS, "newsletter_unsubscribed"),
        SubscriberStatus::Pending => (now + 30 * DAY_SECS, "newsletter_pending"),
    }
}

impl Entity {
    pub const PER_PAGE: u64 = 20;

    #[instrument(skip(conn, new_subscriber), fields(subscriber_id, email = %new_subscriber.email))]
    pub async fn create(conn: &DbConn, new_subscriber: NewSubscriber) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();
        let (expires_at, retention_class) = retention_for(new_subscriber.status, now.timestamp());

        match Self::find()
            .filter(Column::Email.eq(new_subscriber.email.clone()))
            .one(conn)
            .await
        {
            Ok(Some(existing)) => {
                let mut am: ActiveModel = existing.into();
                am.token = Set(new_subscriber.token);
                am.status = Set(new_subscriber.status);
                am.expires_at = Set(expires_at);
                am.retention_class = Set(retention_class.to_string());
                am.updated_at = Set(now);
                match am.update(conn).await {
                    Ok(updated) => {
                        tracing::Span::current().record("subscriber_id", updated.id);
                        info!(subscriber_id = updated.id, "Newsletter subscriber updated");
                        Ok(updated)
                    }
                    Err(err) => {
                        error!("Failed to update newsletter subscriber: {}", err);
                        Err(err.into())
                    }
                }
            }
            Ok(None) => {
                let model = ActiveModel {
                    email: Set(new_subscriber.email),
                    status: Set(new_subscriber.status),
                    token: Set(new_subscriber.token),
                    expires_at: Set(expires_at),
                    retention_class: Set(retention_class.to_string()),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                };
                match model.insert(conn).await {
                    Ok(inserted) => {
                        tracing::Span::current().record("subscriber_id", inserted.id);
                        info!(subscriber_id = inserted.id, "Newsletter subscriber created");
                        Ok(inserted)
                    }
                    Err(err) => {
                        error!("Failed to create newsletter subscriber: {}", err);
                        Err(err.into())
                    }
                }
            }
            Err(err) => {
                error!("Failed to find newsletter subscriber: {}", err);
                Err(err.into())
            }
        }
    }

    pub async fn confirm(conn: &DbConn, email: &str, token: &str) -> DbResult<Option<Model>> {
        let now = chrono::Utc::now().fixed_offset();

        let sub = Self::find()
            .filter(Condition::all().add(Column::Email.eq(email)))
            .one(conn)
            .await?;

        if let Some(model) = sub {
            // Constant-time compare: a plain `==` leaks token validity via timing, enabling mass confirmation probing.
            use subtle::ConstantTimeEq;
            let token_ok = model.token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() == 1;
            if token_ok && model.status != SubscriberStatus::Unsubscribed {
                let mut am: ActiveModel = model.into();
                let (expires_at, class) =
                    retention_for(SubscriberStatus::Confirmed, now.timestamp());
                am.status = Set(SubscriberStatus::Confirmed);
                am.expires_at = Set(expires_at);
                am.retention_class = Set(class.to_string());
                am.updated_at = Set(now);
                let updated = am.update(conn).await?;
                Ok(Some(updated))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub async fn unsubscribe(
        conn: &DbConn,
        email: &str,
        token: Option<&str>,
    ) -> DbResult<Option<Model>> {
        let now = chrono::Utc::now().fixed_offset();

        let sub = Self::find()
            .filter(Condition::all().add(Column::Email.eq(email)))
            .one(conn)
            .await?;

        if let Some(model) = sub {
            if let Some(t) = token {
                // Constant-time compare — see `confirm` (timing side-channel on token).
                use subtle::ConstantTimeEq;
                let token_ok = model.token.as_bytes().ct_eq(t.as_bytes()).unwrap_u8() == 1;
                if !token_ok {
                    return Ok(None);
                }
            }
            let mut am: ActiveModel = model.into();
            let (expires_at, class) =
                retention_for(SubscriberStatus::Unsubscribed, now.timestamp());
            am.status = Set(SubscriberStatus::Unsubscribed);
            am.expires_at = Set(expires_at);
            am.retention_class = Set(class.to_string());
            am.updated_at = Set(now);
            let updated = am.update(conn).await?;
            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }

    /// One-click unsubscribe by row id (RFC 8058, A29). The caller has already
    /// verified the unsubscribe token for this id. Returns whether a row changed.
    pub async fn unsubscribe_by_id(conn: &DbConn, subscriber_id: i32) -> DbResult<bool> {
        let now = chrono::Utc::now().fixed_offset();
        let Some(model) = Self::find_by_id(subscriber_id).one(conn).await? else {
            return Ok(false);
        };
        let (expires_at, class) = retention_for(SubscriberStatus::Unsubscribed, now.timestamp());
        let mut am: ActiveModel = model.into();
        am.status = Set(SubscriberStatus::Unsubscribed);
        am.expires_at = Set(expires_at);
        am.retention_class = Set(class.to_string());
        am.updated_at = Set(now);
        am.update(conn).await?;
        Ok(true)
    }

    pub async fn find_with_query(
        conn: &DbConn,
        query: SubscriberQuery,
    ) -> DbResult<PaginatedList<SubscriberListItem>> {
        let mut q = Self::find().select_only().columns(vec![
            Column::Id,
            Column::Email,
            Column::Status,
            Column::CreatedAt,
            Column::UpdatedAt,
        ]);

        if let Some(search) = &query.search {
            let pattern = format!("%{}%", search);
            q = q.filter(Column::Email.contains(&pattern));
        }

        if let Some(status) = query.status {
            q = q.filter(Column::Status.eq(status));
        }

        if let Some(ts) = query.created_at_gt {
            q = q.filter(Column::CreatedAt.gt(ts));
        }
        if let Some(ts) = query.created_at_lt {
            q = q.filter(Column::CreatedAt.lt(ts));
        }
        if let Some(ts) = query.updated_at_gt {
            q = q.filter(Column::UpdatedAt.gt(ts));
        }
        if let Some(ts) = query.updated_at_lt {
            q = q.filter(Column::UpdatedAt.lt(ts));
        }

        if let Some(sorts) = query.sorts {
            for sort in sorts {
                let column = match sort.field.as_str() {
                    "email" => Some(Column::Email),
                    "status" => Some(Column::Status),
                    "created_at" => Some(Column::CreatedAt),
                    "updated_at" => Some(Column::UpdatedAt),
                    _ => None,
                };
                if let Some(col) = column {
                    q = q.order_by(col, sort.order);
                }
            }
        } else {
            q = q.order_by(Column::CreatedAt, Order::Desc);
        }

        let page = match query.page {
            Some(p) if p > 0 => p,
            _ => 1,
        };

        let paginator = q
            .into_model::<SubscriberListItem>()
            .paginate(conn, Self::PER_PAGE);
        let total = paginator.num_items().await?;
        let items = paginator.fetch_page(page - 1).await?;

        Ok(PaginatedList::new(items, total, page, Self::PER_PAGE))
    }

    pub async fn find_by_id_with_404(conn: &DbConn, subscriber_id: i32) -> DbResult<Model> {
        match Self::find_by_id(subscriber_id).one(conn).await {
            Ok(Some(model)) => Ok(model),
            Ok(None) => Err(ErrorResponse::new(ErrorCode::SubscriberNotFound)
                .with_message(format!("Subscriber with ID {} not found", subscriber_id))),
            Err(err) => Err(err.into()),
        }
    }

    #[instrument(skip(conn, update), fields(subscriber_id))]
    pub async fn update(
        conn: &DbConn,
        subscriber_id: i32,
        update: UpdateSubscriber,
    ) -> DbResult<Option<Model>> {
        let sub: Option<Model> = Self::find_by_id(subscriber_id).one(conn).await?;

        if let Some(model) = sub {
            let mut am: ActiveModel = model.into();

            if let Some(status) = update.status {
                let (expires_at, class) = retention_for(status, update.updated_at.timestamp());
                am.status = Set(status);
                am.expires_at = Set(expires_at);
                am.retention_class = Set(class.to_string());
            }
            if let Some(token) = update.token {
                am.token = Set(token);
            }
            am.updated_at = Set(update.updated_at);

            match am.update(conn).await {
                Ok(updated) => {
                    info!(subscriber_id, "Newsletter subscriber updated");
                    Ok(Some(updated))
                }
                Err(err) => {
                    error!(subscriber_id, "Failed to update subscriber: {}", err);
                    Err(err.into())
                }
            }
        } else {
            warn!(subscriber_id, "Subscriber not found for update");
            Ok(None)
        }
    }

    pub async fn update_by_email(
        conn: &DbConn,
        email: &str,
        update: UpdateSubscriber,
    ) -> DbResult<Option<Model>> {
        let sub = Self::find()
            .filter(Column::Email.eq(email))
            .one(conn)
            .await?;

        if let Some(model) = sub {
            let mut am: ActiveModel = model.into();

            if let Some(status) = update.status {
                let (expires_at, class) = retention_for(status, update.updated_at.timestamp());
                am.status = Set(status);
                am.expires_at = Set(expires_at);
                am.retention_class = Set(class.to_string());
            }
            if let Some(token) = update.token {
                am.token = Set(token);
            }
            am.updated_at = Set(update.updated_at);

            let updated = am.update(conn).await?;
            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }
}
