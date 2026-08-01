use std::collections::{HashMap, HashSet};

use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};

use crate::db::sea_models::{post_access, post_purchase, subscription};
use crate::error::{DbResult, ErrorCode, ErrorResponse};

pub use ruxlog_types::enums::PostAccessType;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostAccessPolicy {
    pub access_type: PostAccessType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_cents: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

impl PostAccessPolicy {
    pub fn free() -> Self {
        Self {
            access_type: PostAccessType::Free,
            price_cents: None,
            currency: None,
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(self.access_type, PostAccessType::Free)
    }
}

impl From<post_access::model::Model> for PostAccessPolicy {
    fn from(m: post_access::model::Model) -> Self {
        Self {
            access_type: m.access_type,
            price_cents: m.price_cents,
            currency: m.currency,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AccessOutcome {
    pub policy: PostAccessPolicy,
    pub granted: bool,
}

pub fn decide_access(
    policy: &PostAccessPolicy,
    viewer_bypasses: bool,
    has_purchase: bool,
    has_active_subscription: bool,
) -> bool {
    if viewer_bypasses || policy.is_open() {
        return true;
    }
    match policy.access_type {
        PostAccessType::Free => true,
        PostAccessType::Paid => has_purchase,
        PostAccessType::SubscriberOnly => has_active_subscription,
    }
}

pub async fn load_post_access_policy(
    db: &DatabaseConnection,
    post_id: i32,
) -> DbResult<PostAccessPolicy> {
    let row = post_access::Entity::find()
        .filter(post_access::Column::PostId.eq(post_id))
        .one(db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;
    Ok(row
        .map(PostAccessPolicy::from)
        .unwrap_or_else(PostAccessPolicy::free))
}

pub async fn load_post_access_map(
    db: &DatabaseConnection,
    post_ids: &[i32],
) -> DbResult<HashMap<i32, PostAccessPolicy>> {
    let mut map = HashMap::new();
    if post_ids.is_empty() {
        return Ok(map);
    }
    let rows = post_access::Entity::find()
        .filter(post_access::Column::PostId.is_in(post_ids.to_vec()))
        .all(db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;
    for r in rows {
        map.insert(r.post_id, PostAccessPolicy::from(r));
    }
    Ok(map)
}

pub async fn user_has_post_purchase(
    db: &DatabaseConnection,
    user_id: i32,
    post_id: i32,
) -> DbResult<bool> {
    let count = post_purchase::Entity::find()
        .filter(post_purchase::Column::UserId.eq(user_id))
        .filter(post_purchase::Column::PostId.eq(post_id))
        .count(db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;
    Ok(count > 0)
}

pub async fn user_purchased_post_ids(
    db: &DatabaseConnection,
    user_id: i32,
    post_ids: &[i32],
) -> DbResult<HashSet<i32>> {
    let mut owned = HashSet::new();
    if post_ids.is_empty() {
        return Ok(owned);
    }
    let rows = post_purchase::Entity::find()
        .filter(post_purchase::Column::UserId.eq(user_id))
        .filter(post_purchase::Column::PostId.is_in(post_ids.to_vec()))
        .all(db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;
    for r in rows {
        owned.insert(r.post_id);
    }
    Ok(owned)
}

pub async fn user_has_active_subscription(db: &DatabaseConnection, user_id: i32) -> DbResult<bool> {
    use ruxlog_types::enums::SubscriptionStatus;
    let subs = subscription::Entity::find()
        .filter(subscription::Column::UserId.eq(user_id))
        .filter(
            Condition::any()
                .add(subscription::Column::Status.eq(SubscriptionStatus::Active))
                .add(subscription::Column::Status.eq(SubscriptionStatus::Trialing)),
        )
        .all(db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    let now_ts = chrono::Utc::now().timestamp();
    for s in subs {
        // Fail closed: an `Active` row with no `current_period_end` must NOT unlock forever — treat missing end as "not in period" (do not flip this `unwrap_or` to `true`).
        let still_in_period = s
            .current_period_end
            .map(|end| end.timestamp() > now_ts)
            .unwrap_or(false);
        if still_in_period {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn user_has_access(
    db: &DatabaseConnection,
    viewer_id: Option<i32>,
    post_id: i32,
    viewer_bypasses: bool,
) -> DbResult<AccessOutcome> {
    let policy = load_post_access_policy(db, post_id).await?;

    if viewer_bypasses || policy.is_open() {
        return Ok(AccessOutcome {
            policy,
            granted: true,
        });
    }

    let Some(user_id) = viewer_id else {
        return Ok(AccessOutcome {
            policy,
            granted: false,
        });
    };

    let (has_purchase, has_active_sub) = match policy.access_type {
        PostAccessType::Paid => (user_has_post_purchase(db, user_id, post_id).await?, false),
        PostAccessType::SubscriberOnly => (false, user_has_active_subscription(db, user_id).await?),
        PostAccessType::Free => (false, false),
    };

    let granted = decide_access(&policy, viewer_bypasses, has_purchase, has_active_sub);
    Ok(AccessOutcome { policy, granted })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paid(price: i32) -> PostAccessPolicy {
        PostAccessPolicy {
            access_type: PostAccessType::Paid,
            price_cents: Some(price),
            currency: Some("USD".into()),
        }
    }

    fn sub_only() -> PostAccessPolicy {
        PostAccessPolicy {
            access_type: PostAccessType::SubscriberOnly,
            price_cents: None,
            currency: None,
        }
    }

    #[test]
    fn free_is_always_open_and_granted() {
        let policy = PostAccessPolicy::free();
        assert!(policy.is_open());
        assert!(decide_access(&policy, false, false, false));
    }

    #[test]
    fn paid_requires_purchase() {
        let policy = paid(499);
        assert!(!policy.is_open());
        assert!(!decide_access(&policy, false, false, false));
        assert!(decide_access(&policy, false, true, false));
        assert!(!decide_access(&policy, false, false, true));
    }

    #[test]
    fn subscriber_only_requires_active_subscription() {
        let policy = sub_only();
        assert!(!policy.is_open());
        assert!(!decide_access(&policy, false, false, false));
        assert!(!decide_access(&policy, false, true, false));
        assert!(decide_access(&policy, false, false, true));
    }

    #[test]
    fn author_and_staff_bypass_the_paywall() {
        assert!(decide_access(&paid(499), true, false, false));
        assert!(decide_access(&sub_only(), true, false, false));
    }
}
