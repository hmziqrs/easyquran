use crate::error::{DbResult, ErrorCode, ErrorResponse};
use chrono::Utc;
use sea_orm::{entity::prelude::*, Order, QueryOrder, Set};

use super::*;

pub fn encode_credential_id(cred_id: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    URL_SAFE_NO_PAD.encode(cred_id)
}

impl Entity {
    pub async fn create<T: ConnectionTrait>(
        conn: &T,
        user_id: i32,
        passkey: &webauthn_rs::prelude::Passkey,
        device_type: Option<String>,
        transports: Option<Json>,
    ) -> DbResult<Model> {
        let credential_id = encode_credential_id(passkey.cred_id().as_slice());
        let public_key = serde_json::to_vec(passkey).map_err(|err| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Failed to serialize passkey")
                .with_details(err.to_string())
        })?;
        let now = Utc::now().fixed_offset();

        let model = ActiveModel {
            user_id: Set(user_id),
            credential_id: Set(credential_id),
            public_key: Set(public_key),
            counter: Set(0),
            device_type: Set(device_type),
            transports: Set(transports),
            created_at: Set(now),
            last_used_at: Set(None),
            ..Default::default()
        };

        match model.insert(conn).await {
            Ok(m) => Ok(m),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn list_by_user<T: ConnectionTrait>(conn: &T, user_id: i32) -> DbResult<Vec<Model>> {
        match Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::CreatedAt, Order::Asc)
            .all(conn)
            .await
        {
            Ok(rows) => Ok(rows),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn find_by_credential_id<T: ConnectionTrait>(
        conn: &T,
        credential_id: &str,
    ) -> DbResult<Option<Model>> {
        match Self::find()
            .filter(Column::CredentialId.eq(credential_id))
            .one(conn)
            .await
        {
            Ok(opt) => Ok(opt),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn touch_counter<T: ConnectionTrait>(
        conn: &T,
        id: i32,
        new_counter: u32,
    ) -> DbResult<()> {
        let now = Utc::now().fixed_offset();
        let mut active: ActiveModel = Self::find()
            .filter(Column::Id.eq(id))
            .one(conn)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::RecordNotFound)
                    .with_message("Passkey credential not found")
            })?
            .into();
        active.counter = Set(new_counter as i64);
        active.last_used_at = Set(Some(now));
        active
            .update(conn)
            .await
            .map(|_| ())
            .map_err(ErrorResponse::from)
    }

    pub async fn delete_by_credential_id_for_user<T: ConnectionTrait>(
        conn: &T,
        credential_id: &str,
        user_id: i32,
    ) -> DbResult<u64> {
        match Self::delete_many()
            .filter(Column::CredentialId.eq(credential_id))
            .filter(Column::UserId.eq(user_id))
            .exec(conn)
            .await
        {
            Ok(res) => Ok(res.rows_affected),
            Err(err) => Err(err.into()),
        }
    }
}
