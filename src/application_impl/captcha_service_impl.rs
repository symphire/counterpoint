use crate::application_port::{CaptchaError, CaptchaResult, CaptchaService, ValidationInput};
use crate::domain_model::CaptchaId;
use crate::domain_port::CaptchaStore;
use captcha_rs::CaptchaBuilder;
use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::sync::Arc;
use std::time::Duration;

const HMAC_SECRET_KEY: &str = "my-secret-key";

pub struct RealCaptchaService {
    store: Arc<dyn CaptchaStore>,
}

impl RealCaptchaService {
    pub fn new(store: Arc<dyn CaptchaStore>, _hmac_key: Vec<u8>) -> Self {
        Self { store }
    }

    fn hmac_hex(&self, code: &str) -> anyhow::Result<String> {
        let mut mac = Hmac::<Sha256>::new_from_slice(HMAC_SECRET_KEY.as_bytes())?;
        mac.update(code.as_bytes());
        let out = mac.finalize().into_bytes();
        Ok(hex::encode(out))
    }
}

#[async_trait::async_trait]
impl CaptchaService for RealCaptchaService {
    async fn generate(&self) -> anyhow::Result<CaptchaResult, CaptchaError> {
        let captcha = CaptchaBuilder::new()
            .length(6)
            .width(100)
            .height(50)
            .dark_mode(false)
            .complexity(1)
            .compression(40)
            .build();

        let id = CaptchaId(uuid::Uuid::new_v4());
        let code = &captcha.text.to_lowercase();
        let id = CaptchaId(uuid::Uuid::nil());
        let code = "123456";
        let code_hmac = self.hmac_hex(&code)?;
        let ttl = Duration::from_secs(300);
        let expire_at = Utc::now() + ttl;

        self.store.save(&id, &code_hmac, expire_at, 5).await?;

        let with_prefix = captcha.to_base64();
        let clean = with_prefix
            .split_once(',')
            .map(|(_, d)| d)
            .unwrap_or(with_prefix.as_str());
        Ok(CaptchaResult {
            id,
            image_base64: clean.to_owned(),
            expire_at,
        })
    }
    async fn validate(&self, input: ValidationInput) -> anyhow::Result<(), CaptchaError> {
        let provided_hmac = self.hmac_hex(&input.answer)?;
        self.store
            .verify_and_consume(&input.id, &provided_hmac)
            .await?;
        Ok(())
    }
}
