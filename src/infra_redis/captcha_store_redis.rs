use crate::domain_model::CaptchaId;
use crate::domain_port::*;
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Script};
const CAPTCHA_VALIDATE: &str = include_str!("captcha_validate.lua");

pub struct RedisCaptchaStore {
    conn: ConnectionManager,
    prefix: String,
}

impl RedisCaptchaStore {
    pub fn new(conn: ConnectionManager, prefix: String) -> Self {
        RedisCaptchaStore { conn, prefix }
    }

    fn key(&self, id: &CaptchaId) -> String {
        format!("{}:{}", self.prefix, id)
    }
}

#[async_trait::async_trait]
impl CaptchaStore for RedisCaptchaStore {
    async fn save(
        &self,
        id: &CaptchaId,
        code_hash_hex: &str,
        expire_at: DateTime<Utc>,
        max_attempts: u32,
    ) -> Result<(), CaptchaStoreError> {
        let key = &self.key(id);
        let mut conn = self.conn.clone();

        let _: () = conn
            .hset(&key, "h", code_hash_hex)
            .await
            .map_err(|e| CaptchaStoreError::Store(e.to_string()))?;
        let _: () = conn
            .hset(&key, "tries", max_attempts as i64)
            .await
            .map_err(|e| CaptchaStoreError::Store(e.to_string()))?;
        let _: () = conn
            .expire_at(&key, expire_at.timestamp())
            .await
            .map_err(|e| CaptchaStoreError::Store(e.to_string()))?;

        Ok(())
    }

    async fn verify_and_consume(
        &self,
        id: &CaptchaId,
        provided_hash_hex: &str,
    ) -> Result<(), CaptchaStoreError> {
        let key = &self.key(id);
        let mut conn = self.conn.clone();
        let script = Script::new(CAPTCHA_VALIDATE);
        let (status, left): (i64, i64) = script
            .key(key)
            .arg(provided_hash_hex)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| CaptchaStoreError::Store(e.to_string()))?;

        match status {
            1 => Ok(()),
            -1 => Err(CaptchaStoreError::NotFoundOrExpired),
            0 => Err(CaptchaStoreError::Incorrect {
                remaining_attempts: left as u32,
            }),
            _ => Err(CaptchaStoreError::InternalError(anyhow!(
                "unknown script status"
            ))),
        }
    }
}
