use crate::error::{DbResult, ErrorCode, ErrorResponse};
use ruxlog_types::PaginatedList;
use sea_orm::entity::prelude::*;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, IntoActiveModel, Order, QueryOrder, QuerySelect, Statement,
};
use tracing::{instrument, warn};

use super::{
    Column, Entity, Model, NewSuppression, SuppressionListItem, SuppressionQuery,
    SuppressionReason, SuppressionUpsert,
};

fn reason_str(r: SuppressionReason) -> &'static str {
    match r {
        SuppressionReason::Bounce => "bounce",
        SuppressionReason::Complaint => "complaint",
        SuppressionReason::Manual => "manual",
    }
}

impl Entity {
    pub const PER_PAGE: u64 = 50;

    #[instrument(skip(conn), fields(recipient))]
    pub async fn find_by_recipient(conn: &DbConn, recipient: &str) -> DbResult<Option<Model>> {
        Self::find()
            .filter(Column::Recipient.eq(recipient))
            .one(conn)
            .await
            .map_err(Into::into)
    }

    /// Server-side never-downgrade UPSERT: a read-then-write races under concurrent webhooks and could let a soft bounce downgrade a permanent complaint, re-enabling a suppressed recipient.
    #[instrument(skip(conn), fields(recipient))]
    pub async fn upsert(conn: &DbConn, recipient: &str, up: SuppressionUpsert) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();
        let perm = up.permanent || up.reason != SuppressionReason::Bounce;

        // blocks_transactional only ratchets up: a webhook repeat (false) never clears a
        // row an admin or a send-time bounce already made blocking.
        const SQL: &str = r#"INSERT INTO email_suppression
            (recipient, reason, source, diagnostic, permanent, blocks_transactional, last_seen, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (recipient) DO UPDATE SET
                permanent = email_suppression.permanent OR EXCLUDED.permanent,
                blocks_transactional = email_suppression.blocks_transactional OR EXCLUDED.blocks_transactional,
                reason = CASE WHEN email_suppression.permanent
                              THEN email_suppression.reason ELSE EXCLUDED.reason END,
                source = EXCLUDED.source,
                diagnostic = EXCLUDED.diagnostic,
                last_seen = EXCLUDED.last_seen,
                updated_at = EXCLUDED.updated_at"#;
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            SQL,
            vec![
                recipient.into(),
                reason_str(up.reason).into(),
                up.source.into(),
                up.diagnostic.into(),
                perm.into(),
                up.blocks_transactional.into(),
                now.into(),
                now.into(),
                now.into(),
            ],
        ))
        .await?;

        Self::find_by_recipient(conn, recipient)
            .await?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::IntegrityError)
                    .with_message("email_suppression upsert returned no row")
            })
    }

    pub async fn create(conn: &DbConn, new: NewSuppression) -> DbResult<Model> {
        let recipient = new.recipient.trim().to_lowercase();
        if recipient.is_empty() {
            return Err(ErrorResponse::new(ErrorCode::InvalidEmailFormat)
                .with_message("Recipient cannot be empty"));
        }
        Self::upsert(
            conn,
            &recipient,
            SuppressionUpsert {
                reason: new.reason,
                source: new.source.or_else(|| Some("admin".to_string())),
                diagnostic: new.diagnostic,
                permanent: new.permanent,
                blocks_transactional: true,
            },
        )
        .await
    }

    /// Admin review of a webhook-sourced row (A29): the row starts blocking transactional mail.
    pub async fn review(conn: &DbConn, recipient: &str) -> DbResult<Option<Model>> {
        let Some(row) = Self::find_by_recipient(conn, recipient).await? else {
            return Ok(None);
        };
        let mut am = row.into_active_model();
        am.blocks_transactional = sea_orm::Set(true);
        am.updated_at = sea_orm::Set(chrono::Utc::now().fixed_offset());
        Ok(Some(am.update(conn).await?))
    }

    pub async fn delete_by_recipient(conn: &DbConn, recipient: &str) -> DbResult<bool> {
        let res = Self::delete_many()
            .filter(Column::Recipient.eq(recipient))
            .exec(conn)
            .await?;
        Ok(res.rows_affected > 0)
    }

    pub async fn find_with_query(
        conn: &DbConn,
        query: SuppressionQuery,
    ) -> DbResult<PaginatedList<SuppressionListItem>> {
        let mut q = Self::find().select_only().columns([
            Column::Id,
            Column::Recipient,
            Column::Reason,
            Column::Permanent,
            Column::BlocksTransactional,
            Column::Source,
            Column::LastSeen,
            Column::CreatedAt,
        ]);

        if let Some(reason) = query.reason {
            q = q.filter(Column::Reason.eq(reason));
        }
        if let Some(permanent) = query.permanent {
            q = q.filter(Column::Permanent.eq(permanent));
        }
        if let Some(search) = &query.search {
            q = q.filter(Column::Recipient.contains(search.as_str()));
        }

        q = q.order_by(Column::LastSeen, Order::Desc);

        let page = match query.page {
            Some(p) if p > 0 => p,
            _ => 1,
        };

        let paginator = q
            .into_model::<SuppressionListItem>()
            .paginate(conn, Self::PER_PAGE);
        let total = paginator.num_items().await?;
        let items = paginator.fetch_page(page - 1).await?;

        Ok(PaginatedList::new(items, total, page, Self::PER_PAGE))
    }

    pub async fn find_by_id_with_404(conn: &DbConn, id: i32) -> DbResult<Model> {
        match Self::find_by_id(id).one(conn).await {
            Ok(Some(model)) => Ok(model),
            Ok(None) => {
                warn!(suppression_id = id, "Suppression entry not found");
                Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                    .with_message(format!("Suppression entry {id} not found")))
            }
            Err(err) => Err(err.into()),
        }
    }
}
