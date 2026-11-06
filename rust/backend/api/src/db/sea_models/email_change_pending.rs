use chrono::Utc;
use sea_orm::{entity::prelude::*, sea_query::OnConflict, ActiveValue::Set};
use subtle::ConstantTimeEq;

use crate::error::DbResult;

/// Lifetime of a parked email change, in seconds.
pub const PENDING_TTL_SECS: i64 = 900;
/// Retention class for `email_change_pending` rows (m000026).
pub const RETENTION_CLASS: &str = "pending_email_change";

/// A requested email change waiting for a code on the new mailbox (A12).
/// `users.email` does not move until that code is proven.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "email_change_pending")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    pub new_email: String,
    pub identity_hash: Option<String>,
    pub code_hash: String,
    pub expires_at: i64,
    pub retention_class: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    /// Parks a change for `user_id`, replacing any earlier one. Replacing is the
    /// typo path: the owner submits the right address and the wrong one is gone.
    pub async fn park<C: ConnectionTrait>(
        conn: &C,
        user_id: i32,
        new_email: &str,
        identity_hash: Option<String>,
        code_hash: String,
    ) -> DbResult<()> {
        let now = Utc::now();
        let row = ActiveModel {
            user_id: Set(user_id),
            new_email: Set(new_email.to_owned()),
            identity_hash: Set(identity_hash),
            code_hash: Set(code_hash),
            expires_at: Set(now.timestamp() + PENDING_TTL_SECS),
            retention_class: Set(RETENTION_CLASS.to_owned()),
            created_at: Set(now.fixed_offset()),
            updated_at: Set(now.fixed_offset()),
        };
        Entity::insert(row)
            .on_conflict(
                OnConflict::column(Column::UserId)
                    .update_columns([
                        Column::NewEmail,
                        Column::IdentityHash,
                        Column::CodeHash,
                        Column::ExpiresAt,
                        Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(conn)
            .await?;
        Ok(())
    }

    /// The live change for `user_id`. An expired row is dropped on read.
    pub async fn live_for<C: ConnectionTrait>(conn: &C, user_id: i32) -> DbResult<Option<Model>> {
        let Some(row) = Entity::find_by_id(user_id).one(conn).await? else {
            return Ok(None);
        };
        if row.expires_at > Utc::now().timestamp() {
            return Ok(Some(row));
        }
        Entity::delete_by_id(user_id).exec(conn).await?;
        Ok(None)
    }

    pub fn code_matches(row: &Model, code_hash: &str) -> bool {
        bool::from(row.code_hash.as_bytes().ct_eq(code_hash.as_bytes()))
    }
}
