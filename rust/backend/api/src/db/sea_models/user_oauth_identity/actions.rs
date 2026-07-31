use crate::error::DbResult;
use sea_orm::{entity::prelude::*, Set};
use tracing::{info, instrument, warn};

use super::*;

impl Entity {
    pub async fn find_by_provider<T: ConnectionTrait>(
        conn: &T,
        provider: &str,
        provider_user_id: &str,
    ) -> DbResult<Option<Model>> {
        match Self::find()
            .filter(Column::Provider.eq(provider))
            .filter(Column::ProviderUserId.eq(provider_user_id))
            .one(conn)
            .await
        {
            Ok(model) => Ok(model),
            Err(err) => Err(err.into()),
        }
    }

    #[instrument(skip(conn), fields(user_id = new.user_id, provider = %new.provider))]
    pub async fn link<T: ConnectionTrait>(conn: &T, new: NewOauthIdentity) -> DbResult<Model> {
        let active = ActiveModel {
            user_id: Set(new.user_id),
            provider: Set(new.provider),
            provider_user_id: Set(new.provider_user_id),
            created_at: Set(new.created_at),
            ..Default::default()
        };

        match active.insert(conn).await {
            Ok(model) => {
                info!(identity_id = model.id, "Linked OAuth provider identity");
                Ok(model)
            }
            Err(err) => {
                warn!(error = %err, "Failed to link OAuth provider identity");
                Err(err.into())
            }
        }
    }

    pub async fn delete_for_user<T: ConnectionTrait>(conn: &T, user_id: i32) -> DbResult<u64> {
        match Self::delete_many()
            .filter(Column::UserId.eq(user_id))
            .exec(conn)
            .await
        {
            Ok(res) => Ok(res.rows_affected),
            Err(err) => Err(err.into()),
        }
    }
}
