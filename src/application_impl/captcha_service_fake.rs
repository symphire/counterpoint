use crate::application_port::{CaptchaError, CaptchaResult, CaptchaService, ValidationInput};
use crate::domain_model::CaptchaId;
use chrono::Utc;
use std::time::Duration;

/// Base64-encoded JPEG image displaying "123456" with dimensions 100x50px.
const FAKE_CAPTCHA_BASE64: &str = "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";

#[derive(Debug)]
pub struct FakeCaptchaService;

impl FakeCaptchaService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl CaptchaService for FakeCaptchaService {
    async fn generate(&self) -> anyhow::Result<CaptchaResult, CaptchaError> {
        let ttl = Duration::from_secs(300);
        Ok(CaptchaResult {
            id: CaptchaId(uuid::Uuid::nil()),
            image_base64: FAKE_CAPTCHA_BASE64.to_string(),
            expire_at: Utc::now() + ttl,
        })
    }

    async fn validate(&self, input: ValidationInput) -> anyhow::Result<(), CaptchaError> {
        match input.answer.as_str() {
            "1" => Ok(()),
            "123456" => Ok(()),
            "000000" => Err(CaptchaError::InternalError(anyhow::anyhow!(
                "Simulated internal error"
            ))),
            _ => Err(CaptchaError::NotFoundOrExpired),
        }
    }
}
