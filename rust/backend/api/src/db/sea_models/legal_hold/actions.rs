use chrono::{DateTime, Utc};
use sea_orm::{
    entity::prelude::*, sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::error::DbResult;

use super::{model, subject};

const DAY_SECS: i64 = 24 * 60 * 60;
/// CSAM hold window. Never auto-released; the window only sets the first expiry.
const CSAM_SECS: i64 = 365 * DAY_SECS;
/// Law-enforcement preservation window. Each renewal adds one window.
const LAW_ENFORCEMENT_SECS: i64 = 90 * DAY_SECS;
/// Enforcement record window.
const ENFORCEMENT_SECS: i64 = 365 * DAY_SECS;

pub const CLASS_CSAM_HOLD_1Y: &str = "csam_hold_1y";
pub const CLASS_LAW_ENFORCEMENT_90D: &str = "law_enforcement_90d";
pub const CLASS_ENFORCEMENT_12M: &str = "enforcement_12m";

/// The three hold rules. The class is stored as `legal_holds.retention_class`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HoldRule {
    CsamHold1y,
    LawEnforcement90d,
    Enforcement12m,
}

impl HoldRule {
    pub fn retention_class(self) -> &'static str {
        match self {
            Self::CsamHold1y => CLASS_CSAM_HOLD_1Y,
            Self::LawEnforcement90d => CLASS_LAW_ENFORCEMENT_90D,
            Self::Enforcement12m => CLASS_ENFORCEMENT_12M,
        }
    }

    /// Only the CSAM hold is exempt from automatic release by expiry.
    pub fn auto_release(self) -> bool {
        !matches!(self, Self::CsamHold1y)
    }

    fn window_secs(self) -> i64 {
        match self {
            Self::CsamHold1y => CSAM_SECS,
            Self::LawEnforcement90d => LAW_ENFORCEMENT_SECS,
            Self::Enforcement12m => ENFORCEMENT_SECS,
        }
    }

    pub fn renewable(self) -> bool {
        matches!(self, Self::LawEnforcement90d)
    }
}

pub struct HoldSubject {
    pub kind: String,
    pub id: String,
    pub user_id: Option<i32>,
}

pub struct NewHold {
    pub rule: HoldRule,
    pub authority: String,
    pub reason: String,
    pub owner: String,
    pub case_ref: Option<String>,
    pub subjects: Vec<HoldSubject>,
}

/// Insert a hold and its subjects. Caller owns the transaction. Expiry is
/// `now + window`; the law-enforcement hold also sets `preserve_until` to it.
pub async fn place<C: ConnectionTrait>(
    conn: &C,
    new: NewHold,
    now: DateTime<Utc>,
) -> DbResult<model::Model> {
    let placed_at = now.timestamp();
    let expires_at = placed_at + new.rule.window_secs();
    let preserve_until = new.rule.renewable().then_some(expires_at);
    let hold = model::ActiveModel {
        case_ref: Set(new.case_ref),
        authority: Set(new.authority),
        reason: Set(new.reason),
        owner: Set(new.owner),
        auto_release: Set(new.rule.auto_release()),
        placed_at: Set(placed_at),
        preserve_until: Set(preserve_until),
        renewals: Set(0),
        released_at: Set(None),
        expires_at: Set(expires_at),
        retention_class: Set(new.rule.retention_class().to_string()),
        ..Default::default()
    }
    .insert(conn)
    .await?;

    for s in new.subjects {
        subject::ActiveModel {
            hold_id: Set(hold.id),
            subject_kind: Set(s.kind),
            subject_id: Set(s.id),
            subject_user_id: Set(s.user_id),
            expires_at: Set(expires_at),
            retention_class: Set(new.rule.retention_class().to_string()),
            ..Default::default()
        }
        .insert(conn)
        .await?;
    }
    Ok(hold)
}

/// Extend a renewable hold by one window from the later of its current
/// `preserve_until` and `now`. Caller owns the transaction. Returns `None` when
/// the hold is missing, released, or not renewable.
pub async fn renew<C: ConnectionTrait>(
    conn: &C,
    hold_id: i64,
    now: DateTime<Utc>,
) -> DbResult<Option<model::Model>> {
    let Some(existing) = model::Entity::find_by_id(hold_id).one(conn).await? else {
        return Ok(None);
    };
    if existing.released_at.is_some() || existing.retention_class != CLASS_LAW_ENFORCEMENT_90D {
        return Ok(None);
    }
    let base = existing.preserve_until.unwrap_or(0).max(now.timestamp());
    let until = base + LAW_ENFORCEMENT_SECS;
    let renewals = existing.renewals + 1;
    let mut active: model::ActiveModel = existing.into();
    active.preserve_until = Set(Some(until));
    active.expires_at = Set(until);
    active.renewals = Set(renewals);
    let updated = active.update(conn).await?;

    subject::Entity::update_many()
        .col_expr(subject::Column::ExpiresAt, Expr::value(until))
        .filter(subject::Column::HoldId.eq(hold_id))
        .exec(conn)
        .await?;
    Ok(Some(updated))
}

/// Subjects copied into a hold, in insert order.
pub async fn subjects_for<C: ConnectionTrait>(
    conn: &C,
    hold_id: i64,
) -> DbResult<Vec<subject::Model>> {
    Ok(subject::Entity::find()
        .filter(subject::Column::HoldId.eq(hold_id))
        .order_by_asc(subject::Column::Id)
        .all(conn)
        .await?)
}
