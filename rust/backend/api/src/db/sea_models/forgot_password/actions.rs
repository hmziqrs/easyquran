use crate::{
    db::sea_models::user,
    error::{DbResult, ErrorCode, ErrorResponse},
};
use chrono::Utc;
use ruxlog_types::PaginatedList;
use sea_orm::{
    entity::prelude::*, IntoActiveModel, JoinType, Order, QueryOrder, QuerySelect, Set,
    TransactionTrait,
};

use super::*;

const ADMIN_PER_PAGE: u64 = 20;

impl Entity {
    pub async fn create<T: ConnectionTrait>(
        conn: &T,
        new_forgot_password: NewForgotPassword,
    ) -> DbResult<Model> {
        let now = Utc::now().fixed_offset();
        let forgot_password = ActiveModel {
            user_id: Set(new_forgot_password.user_id),
            code_hash: Set(new_forgot_password.code_hash),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        match forgot_password.insert(conn).await {
            Ok(model) => Ok(model),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn find_query<T: ConnectionTrait>(
        conn: &T,
        user_id: Option<i32>,
        email: Option<&str>,
        code_hash: Option<&str>,
    ) -> DbResult<Model> {
        if user_id.is_none() && email.is_none() && code_hash.is_none() {
            return Err(ErrorResponse::new(ErrorCode::InvalidInput)
                .with_message("Either user_id, email or code must be provided"));
        }
        let mut query = Self::find();

        if let Some(user_id) = user_id {
            query = query.filter(Column::UserId.eq(user_id));
        }
        if let Some(code_hash) = code_hash {
            query = query.filter(Column::CodeHash.eq(code_hash));
        }
        if let Some(email) = email {
            query = query
                .join(JoinType::InnerJoin, Relation::User.def())
                .filter(user::Column::Email.eq(email));
        }
        match query.one(conn).await {
            Ok(Some(result)) => Ok(result),
            Ok(None) => Err(ErrorResponse::new(ErrorCode::InvalidInput)
                .with_message("The provided verification code is invalid")),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn verify_code(conn: &DbConn, email: &str, code_hash: &str) -> DbResult<user::Model> {
        let transaction = conn.begin().await?;
        let verification =
            Self::find_query(&transaction, None, Some(email), Some(code_hash)).await?;
        let account = user::Entity::find_by_id_with_404(&transaction, verification.user_id).await?;
        if Self::consume_code(&transaction, &verification).await? != 1 {
            return Err(ErrorResponse::new(ErrorCode::InvalidInput)
                .with_message("The verification code is invalid or has expired"));
        }
        transaction.commit().await?;
        Ok(account)
    }

    pub async fn reset(
        conn: &DbConn,
        user_id: i32,
        password: String,
        session_auth_secret: &str,
    ) -> DbResult<u64> {
        let trx = conn.begin().await?;
        let account = user::Entity::find_by_id_with_404(&trx, user_id).await?;
        use subtle::ConstantTimeEq;
        if !bool::from(
            account
                .session_auth_secret
                .as_bytes()
                .ct_eq(session_auth_secret.as_bytes()),
        ) {
            return Err(ErrorResponse::new(ErrorCode::InvalidInput)
                .with_message("Reset token is invalid or has expired"));
        }

        let delete_query = Self::delete_many()
            .filter(Column::UserId.eq(user_id))
            .exec(&trx)
            .await;

        match delete_query {
            Ok(_) => match user::Entity::change_password(&trx, user_id, password).await {
                Ok(_) => {
                    trx.commit().await?;
                }
                Err(err) => {
                    trx.rollback().await?;
                    return Err(err);
                }
            },
            Err(err) => {
                trx.rollback().await?;
                return Err(err.into());
            }
        };
        Ok(1)
    }

    pub async fn consume_code<T: ConnectionTrait>(conn: &T, verification: &Model) -> DbResult<u64> {
        Self::delete_many()
            .filter(Column::Id.eq(verification.id))
            .filter(Column::UserId.eq(verification.user_id))
            .filter(Column::CodeHash.eq(&verification.code_hash))
            .filter(Column::UpdatedAt.eq(verification.updated_at))
            .filter(Column::UpdatedAt.gt(Utc::now().fixed_offset() - Self::EXPIRY_TIME))
            .exec(conn)
            .await
            .map(|r| r.rows_affected)
            .map_err(Into::into)
    }

    pub async fn regenerate_for_email(
        conn: &DbConn,
        user_id: i32,
        email: &str,
        code_hash: String,
    ) -> DbResult<Model> {
        let transaction = conn.begin().await?;
        let account = user::Entity::find_by_id_with_404(&transaction, user_id).await?;
        if account.email != email {
            return Err(ErrorResponse::new(ErrorCode::InvalidInput)
                .with_message("Account email changed; request a new reset code"));
        }
        let verification = Self::regenerate(&transaction, user_id, code_hash).await?;
        transaction.commit().await?;
        Ok(verification)
    }

    pub async fn regenerate<T: ConnectionTrait>(
        conn: &T,
        user_id: i32,
        code_hash: String,
    ) -> DbResult<Model> {
        let now = Utc::now().fixed_offset();

        let existing = Self::find_query(conn, Some(user_id), None, None).await;

        match existing {
            Ok(existing_model) => {
                let mut active_model: ActiveModel = existing_model.into_active_model();
                active_model.code_hash = Set(code_hash);
                active_model.updated_at = Set(now);
                active_model.update(conn).await.map_err(Into::into)
            }
            Err(err) if err.code == ErrorCode::InvalidInput => {
                let new_forgot_password = NewForgotPassword { user_id, code_hash };
                Self::create(conn, new_forgot_password).await
            }
            Err(err) => Err(err),
        }
    }

    pub async fn admin_query(
        conn: &DbConn,
        query: &AdminForgotPasswordQuery,
    ) -> DbResult<PaginatedList<Model>> {
        let mut db_query = Self::find();

        if let Some(user_id) = query.user_id {
            db_query = db_query.filter(Column::UserId.eq(user_id));
        }

        if let Some(code_hash) = &query.code_hash {
            db_query = db_query.filter(Column::CodeHash.eq(code_hash));
        }

        if let Some(created_at) = query.created_at {
            db_query = db_query.filter(Column::CreatedAt.gte(created_at));
        }

        if let Some(updated_at) = query.updated_at {
            db_query = db_query.filter(Column::UpdatedAt.gte(updated_at));
        }

        if let (Some(sort_by), Some(sort_order)) = (&query.sort_by, &query.sort_order) {
            for field in sort_by {
                let order = if sort_order == "asc" {
                    Order::Asc
                } else {
                    Order::Desc
                };

                match field.as_str() {
                    "id" => db_query = db_query.order_by(Column::Id, order),
                    "user_id" => db_query = db_query.order_by(Column::UserId, order),
                    "code_hash" => db_query = db_query.order_by(Column::CodeHash, order),
                    "created_at" => db_query = db_query.order_by(Column::CreatedAt, order),
                    "updated_at" => db_query = db_query.order_by(Column::UpdatedAt, order),
                    _ => {}
                }
            }
        } else {
            db_query = db_query.order_by(Column::Id, Order::Desc);
        }

        let page = match query.page_no {
            Some(p) if p > 0 => p as u64,
            _ => 1,
        };

        let paginator = db_query.paginate(conn, ADMIN_PER_PAGE);

        match paginator.num_items().await {
            Ok(total) => match paginator.fetch_page(page - 1).await {
                Ok(results) => Ok(PaginatedList::new(results, total, page, ADMIN_PER_PAGE)),
                Err(err) => Err(err.into()),
            },
            Err(err) => Err(err.into()),
        }
    }
}
