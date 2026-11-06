//! Goal reminder schedules (F9). One row per account and kind. Times are minutes
//! after local midnight in `time_zone`. Lives as long as the account.

use crate::error::{DbResult, DbResultExt};
use sea_orm::{
    entity::prelude::*,
    sea_query::{Condition, Expr, OnConflict},
    Set,
};

/// The only reminder kind today. A closed set: the API refuses anything else.
pub const KIND_GOAL: &str = "goal";
pub const RETENTION_CLASS: &str = "account_lifetime";
pub const NEVER_EXPIRES: i64 = 253_402_300_799;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "reminder_schedules")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub kind: String,
    pub enabled: bool,
    pub time_zone: String,
    pub reminder_minute: i32,
    pub quiet_start_minute: Option<i32>,
    pub quiet_end_minute: Option<i32>,
    /// Local day (`YYYY-MM-DD` in `time_zone`) the reminder last went out.
    pub last_fired_day: Option<String>,
    pub updated_at: DateTimeWithTimeZone,
    pub expires_at: i64,
    pub retention_class: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// What a settings write sets. `last_fired_day` is never written here.
#[derive(Clone, Debug)]
pub struct NewSchedule {
    pub user_id: i32,
    pub kind: String,
    pub enabled: bool,
    pub time_zone: String,
    pub reminder_minute: i32,
    pub quiet_start_minute: Option<i32>,
    pub quiet_end_minute: Option<i32>,
}

impl Entity {
    pub async fn find_for(conn: &DbConn, user_id: i32, kind: &str) -> DbResult<Option<Model>> {
        Self::find_by_id((user_id, kind.to_string()))
            .one(conn)
            .await
            .map_err_to_response()
    }

    /// Creates or replaces the schedule. `last_fired_day` survives a settings write.
    pub async fn upsert(conn: &DbConn, new: NewSchedule) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();
        let am = ActiveModel {
            user_id: Set(new.user_id),
            kind: Set(new.kind),
            enabled: Set(new.enabled),
            time_zone: Set(new.time_zone),
            reminder_minute: Set(new.reminder_minute),
            quiet_start_minute: Set(new.quiet_start_minute),
            quiet_end_minute: Set(new.quiet_end_minute),
            last_fired_day: Set(None),
            updated_at: Set(now),
            expires_at: Set(NEVER_EXPIRES),
            retention_class: Set(RETENTION_CLASS.to_string()),
        };
        Self::insert(am)
            .on_conflict(
                OnConflict::columns([Column::UserId, Column::Kind])
                    .update_columns([
                        Column::Enabled,
                        Column::TimeZone,
                        Column::ReminderMinute,
                        Column::QuietStartMinute,
                        Column::QuietEndMinute,
                        Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(conn)
            .await
            .map_err_to_response()
    }

    /// Every enabled schedule. The scheduler decides per row whether it is due.
    pub async fn list_enabled(conn: &DbConn) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::Enabled.eq(true))
            .all(conn)
            .await
            .map_err_to_response()
    }

    /// Marks `day` as fired. Returns the rows changed: 1 when this call won the
    /// day, 0 when it already fired or the schedule was switched off meanwhile.
    pub async fn claim_day(conn: &DbConn, user_id: i32, kind: &str, day: &str) -> DbResult<u64> {
        let res = Self::update_many()
            .col_expr(Column::LastFiredDay, Expr::value(day.to_string()))
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Kind.eq(kind))
            .filter(Column::Enabled.eq(true))
            .filter(
                Condition::any()
                    .add(Column::LastFiredDay.is_null())
                    .add(Column::LastFiredDay.ne(day)),
            )
            .exec(conn)
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected)
    }

    /// Undoes a claim after a failed send, so a later tick can retry the same day.
    pub async fn release_day(conn: &DbConn, user_id: i32, kind: &str, day: &str) -> DbResult<()> {
        Self::update_many()
            .col_expr(Column::LastFiredDay, Expr::value(Option::<String>::None))
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Kind.eq(kind))
            .filter(Column::LastFiredDay.eq(day))
            .exec(conn)
            .await
            .map_err_to_response()?;
        Ok(())
    }
}
