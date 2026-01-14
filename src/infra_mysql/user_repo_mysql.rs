use super::util::downcast;
use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::*;
use sqlx::{MySqlPool, Row};

pub struct MySqlUserRepo {
    pool: MySqlPool,
}
impl MySqlUserRepo {
    pub fn new(pool: MySqlPool) -> Self {
        MySqlUserRepo { pool }
    }
}

#[async_trait::async_trait]
impl UserRepo for MySqlUserRepo {
    async fn create_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
        username: &str,
    ) -> Result<(), AuthError> {
        let tx = downcast(tx);

        sqlx::query(
            r#"
INSERT INTO user (user_id, username, is_active)
VALUES (?, ?, ?)
"#,
        )
        .bind(user_id.0.as_bytes() as &[u8])
        .bind(username)
        .bind(true)
        .execute(tx.conn())
        .await
        .map_err(|e| AuthError::Store(e.to_string()))?;

        Ok(())
    }

    async fn get_username_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
    ) -> Result<String, AuthError> {
        let tx = downcast(tx);

        if let Some(row) =
            sqlx::query("SELECT username FROM user WHERE user_id = ? AND is_active = 1")
                .bind(user_id)
                .fetch_optional(tx.conn())
                .await
                .map_err(|e| AuthError::Store(format!("query username: {e}")))?
        {
            return Ok(row.get::<String, _>("username"));
        }

        Err(AuthError::UserNotFound)
    }

    async fn get_id_by_username_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        username: &str,
    ) -> Result<UserId, AuthError> {
        let tx = downcast(tx);

        if let Some(row) =
            sqlx::query("SELECT user_id FROM user WHERE username = ? AND is_active = 1")
                .bind(username)
                .fetch_optional(tx.conn())
                .await
                .map_err(|e| AuthError::Store(format!("query user_id: {e}")))?
        {
            return Ok(row.get::<UserId, _>("user_id"));
        }

        Err(AuthError::UserNotFound)
    }

    async fn username_exists(&self, username: &str) -> Result<bool, AuthError> {
        let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM user WHERE username = ?"#)
            .bind(username)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AuthError::Store(e.to_string()))?;

        Ok(count > 0)
    }

    async fn id_exists(&self, user_id: UserId) -> Result<bool, AuthError> {
        let count: i64 = sqlx::query_scalar(
            r#"
SELECT COUNT(1)
FROM user
WHERE user_id = UUID_TO_BIN(?)
"#,
        )
        .bind(user_id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::Store(e.to_string()))?;

        Ok(count > 0)
    }
}
