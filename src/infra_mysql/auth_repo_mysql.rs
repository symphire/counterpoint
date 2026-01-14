use super::util::downcast;
use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::*;
use chrono::{DateTime, Utc};
use sqlx::mysql::MySqlRow;
use sqlx::{MySqlPool, Row};
use uuid::Uuid;

pub struct MySqlAuthRepo {
    pool: MySqlPool,
}

impl MySqlAuthRepo {
    pub fn new(pool: MySqlPool) -> Self {
        MySqlAuthRepo { pool }
    }

    #[inline]
    fn uid_as_bytes(id: &UserId) -> &[u8] {
        id.0.as_bytes()
    }

    #[inline]
    fn uid_from_bytes(id: &[u8]) -> Result<UserId, AuthError> {
        Ok(UserId(
            Uuid::from_slice(id).map_err(|e| AuthError::Store(e.to_string()))?,
        ))
    }

    fn row_to_record(row: MySqlRow) -> Result<AuthCredentialsRecord, AuthError> {
        let user_id_bytes: Vec<u8> = row
            .try_get("user_id")
            .map_err(|e| AuthError::Store(e.to_string()))?;
        let user_id = Self::uid_from_bytes(&user_id_bytes)?;

        let username: String = row
            .try_get("username")
            .map_err(|e| AuthError::Store(e.to_string()))?;
        let password_hash: String = row
            .try_get("password_hash")
            .map_err(|e| AuthError::Store(e.to_string()))?;
        let is_active: bool = row
            .try_get("is_active")
            .map_err(|e| AuthError::Store(e.to_string()))?;

        let created_at: DateTime<Utc> = row
            .try_get("created_at")
            .map_err(|e| AuthError::Store(e.to_string()))?;

        Ok(AuthCredentialsRecord {
            user_id,
            username,
            password_hash,
            is_active,
            created_at,
        })
    }
}

#[async_trait::async_trait]
impl AuthRepo for MySqlAuthRepo {
    async fn create_credentials_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
        username: &str,
        password_hash: &str,
    ) -> Result<(), AuthError> {
        let tx = downcast(tx);

        sqlx::query(
            r#"
INSERT INTO auth_credential (user_id, username, password_hash)
VALUES (?, ?, ?)
"#,
        )
        .bind(Self::uid_as_bytes(&user_id))
        .bind(username)
        .bind(password_hash)
        .execute(tx.conn())
        .await
        .map_err(|e| AuthError::Store(e.to_string()))?;

        Ok(())
    }

    async fn get_by_username(
        &self,
        username: &str,
    ) -> Result<Option<AuthCredentialsRecord>, AuthError> {
        let row_opt: Option<MySqlRow> = sqlx::query(
            r#"
SELECT user_id, username, password_hash, is_active, created_at
FROM auth_credential
WHERE username = ?
"#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Store(e.to_string()))?;

        row_opt.map(Self::row_to_record).transpose()
    }
}
