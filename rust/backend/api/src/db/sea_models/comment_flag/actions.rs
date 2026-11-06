use crate::error::DbResult;
use ruxlog_types::PaginatedList;
use sea_orm::{
    entity::prelude::*, ColumnTrait, EntityTrait, JoinType, Order, QueryFilter, QueryOrder,
    QuerySelect, Set,
};

use super::{slice::*, *};

/// The comment's last clear instant (A27); `None` when its flags were never cleared.
async fn cleared_at_for<C: ConnectionTrait>(
    conn: &C,
    comment_id: i32,
) -> DbResult<Option<sea_orm::prelude::DateTimeWithTimeZone>> {
    use super::super::post_comment::Entity as PostCommentEntity;
    Ok(PostCommentEntity::find_by_id(comment_id)
        .one(conn)
        .await?
        .and_then(|pc| pc.flags_cleared_at))
}

impl Entity {
    const PER_PAGE: u64 = 20;

    pub async fn create<C: ConnectionTrait>(conn: &C, new_flag: NewCommentFlag) -> DbResult<Model> {
        let existing = Entity::find()
            .filter(Column::CommentId.eq(new_flag.comment_id))
            .filter(Column::UserId.eq(new_flag.user_id))
            .one(conn)
            .await?;

        let now = chrono::Utc::now().fixed_offset();
        let cleared_at = cleared_at_for(conn, new_flag.comment_id).await?;

        let model = if let Some(found) = existing {
            // A flag from before the last clear no longer counts. Flagging again after
            // the clear is a new flag, so its row moves to now and counts again.
            let stale = cleared_at.is_some_and(|at| found.created_at <= at);
            let mut active: ActiveModel = found.into();
            active.reason = Set(new_flag.reason.clone());
            if stale {
                active.created_at = Set(now);
            }
            active.update(conn).await?
        } else {
            let active = ActiveModel {
                comment_id: Set(new_flag.comment_id),
                user_id: Set(new_flag.user_id),
                reason: Set(new_flag.reason.clone()),
                created_at: Set(now),
                ..Default::default()
            };
            active.insert(conn).await?
        };

        let _ = Self::sync_flags_count(conn, new_flag.comment_id).await?;

        Ok(model)
    }

    pub async fn list(
        conn: &DbConn,
        public_url: &str,
        query: CommentFlagQuery,
    ) -> DbResult<PaginatedList<FlagWithUser>> {
        use super::super::media::url::public_file_url_expr;
        use super::super::user::Column as UserColumn;
        use sea_orm::prelude::Expr;
        use sea_orm::sea_query::Alias;

        let mut q = Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::CommentId)
            .column(Column::UserId)
            .column(Column::Reason)
            .column(Column::CreatedAt)
            .column_as(UserColumn::Name, "user_name")
            .column_as(UserColumn::AvatarId, "user_avatar_id")
            .join(JoinType::InnerJoin, Relation::User.def())
            .join_as(
                JoinType::LeftJoin,
                super::super::user::Relation::Media.def(),
                Alias::new("user_avatar_media"),
            )
            .expr_as(
                Expr::col((
                    Alias::new("user_avatar_media"),
                    super::super::media::Column::ObjectKey,
                )),
                "user_avatar_object_key",
            )
            .expr_as(
                public_file_url_expr(public_url, "user_avatar_media"),
                "user_avatar_file_url",
            )
            .expr_as(
                Expr::col((
                    Alias::new("user_avatar_media"),
                    super::super::media::Column::MimeType,
                )),
                "user_avatar_mime_type",
            )
            .expr_as(
                Expr::col((
                    Alias::new("user_avatar_media"),
                    super::super::media::Column::Width,
                )),
                "user_avatar_width",
            )
            .expr_as(
                Expr::col((
                    Alias::new("user_avatar_media"),
                    super::super::media::Column::Height,
                )),
                "user_avatar_height",
            )
            .expr_as(
                Expr::col((
                    Alias::new("user_avatar_media"),
                    super::super::media::Column::Size,
                )),
                "user_avatar_size",
            );

        if let Some(comment_id) = query.comment_id {
            q = q.filter(Column::CommentId.eq(comment_id));
        }
        if let Some(user_id) = query.user_id {
            q = q.filter(Column::UserId.eq(user_id));
        }
        if let Some(term) = &query.search_term {
            q = q.filter(Column::Reason.contains(term));
        }

        let order = if query.sort_order.as_deref() == Some("asc") {
            Order::Asc
        } else {
            Order::Desc
        };

        q = match &query.sort_by {
            Some(fields) if !fields.is_empty() => match fields[0].as_str() {
                "created_at" => q.order_by(Column::CreatedAt, order),
                _ => q.order_by(Column::CreatedAt, order),
            },
            _ => q.order_by(Column::CreatedAt, order),
        };

        let page = match query.page_no {
            Some(p) if p > 0 => p,
            _ => 1,
        };

        let paginator = q
            .into_model::<FlagWithUserJoined>()
            .paginate(conn, Self::PER_PAGE);
        let total = paginator.num_items().await?;
        let items_joined = paginator.fetch_page(page - 1).await?;

        let items = items_joined
            .into_iter()
            .map(|f| f.into_flag_with_user())
            .collect();

        Ok(PaginatedList::new(items, total, page, Self::PER_PAGE))
    }

    pub async fn summary_for_comment(conn: &DbConn, comment_id: i32) -> DbResult<FlagsSummary> {
        let count = Entity::find()
            .filter(Column::CommentId.eq(comment_id))
            .count(conn)
            .await?;

        Ok(FlagsSummary {
            comment_id,
            flags_count: count as i64,
        })
    }

    /// Flag rows still on a comment. Any row counts as open evidence: flags-clear
    /// zeroes the cached count but keeps the rows (A25), so the removal guard holds.
    pub async fn count_for_comment<C: ConnectionTrait>(conn: &C, comment_id: i32) -> DbResult<u64> {
        Ok(Entity::find()
            .filter(Column::CommentId.eq(comment_id))
            .count(conn)
            .await?)
    }

    /// Cached flag count: rows created after the comment's `flags_cleared_at` (A27).
    /// Compared in Rust on the stored instants, so the same clock format is used on
    /// both sides and sub-second order holds.
    pub async fn sync_flags_count<C: ConnectionTrait>(conn: &C, comment_id: i32) -> DbResult<i64> {
        use super::super::post_comment::Entity as PostCommentEntity;

        let comment = PostCommentEntity::find_by_id(comment_id).one(conn).await?;
        let cleared_at = comment.as_ref().and_then(|pc| pc.flags_cleared_at);
        let flags = Entity::find()
            .filter(Column::CommentId.eq(comment_id))
            .all(conn)
            .await?;
        let count = flags
            .iter()
            .filter(|flag| cleared_at.is_none_or(|at| flag.created_at > at))
            .count() as i64;

        if let Some(pc) = comment {
            let mut active: super::super::post_comment::ActiveModel = pc.into();
            active.flags_count = Set(count as i32);
            active.updated_at = Set(chrono::Utc::now().fixed_offset());
            let _ = active.update(conn).await?;
        }
        Ok(count)
    }
}
