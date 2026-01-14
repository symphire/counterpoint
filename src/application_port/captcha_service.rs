use crate::domain_model::CaptchaId;
use crate::domain_port::CaptchaStoreError;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct CaptchaResult {
    pub id: CaptchaId,
    pub image_base64: String,
    pub expire_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ValidationInput {
    pub id: CaptchaId,
    pub answer: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptchaError {
    #[error("incorrect code, {remaining_attempts} attempt(s) left")]
    Incorrect { remaining_attempts: u32 },
    #[error("Captcha not found or expired")]
    NotFoundOrExpired,
    #[error("infra error: {0}")]
    Store(String),
    #[error("Internal error: {0}")]
    InternalError(#[from] anyhow::Error),
}

impl From<CaptchaStoreError> for CaptchaError {
    fn from(err: CaptchaStoreError) -> Self {
        match err {
            CaptchaStoreError::Incorrect {
                remaining_attempts: retry,
            } => CaptchaError::Incorrect {
                remaining_attempts: retry,
            },
            CaptchaStoreError::NotFoundOrExpired => CaptchaError::NotFoundOrExpired,
            CaptchaStoreError::Store(e) => CaptchaError::Store(e),
            CaptchaStoreError::InternalError(e) => CaptchaError::InternalError(e),
        }
    }
}

#[async_trait::async_trait]
pub trait CaptchaService: Send + Sync {
    async fn generate(&self) -> Result<CaptchaResult, CaptchaError>;
    async fn validate(&self, input: ValidationInput) -> Result<(), CaptchaError>;
}
