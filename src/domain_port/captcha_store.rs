use crate::domain_model::CaptchaId;
use chrono::{DateTime, Utc};

#[async_trait::async_trait]
pub trait CaptchaStore: Send + Sync {
    async fn save(
        &self,
        id: &CaptchaId,
        code_hash_hex: &str,
        expire_at: DateTime<Utc>,
        max_attempts: u32,
    ) -> Result<(), CaptchaStoreError>;

    async fn verify_and_consume(
        &self,
        id: &CaptchaId,
        provided_hash_hex: &str,
    ) -> Result<(), CaptchaStoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CaptchaStoreError {
    #[error("incorrect code, {remaining_attempts} attempt(s) left")]
    Incorrect { remaining_attempts: u32 },
    #[error("Captcha not found or expired")]
    NotFoundOrExpired,
    #[error("infra error: {0}")]
    Store(String),
    #[error("Internal error: {0}")]
    InternalError(#[from] anyhow::Error),
}
