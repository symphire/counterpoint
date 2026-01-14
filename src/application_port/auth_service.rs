use crate::domain_model::UserId;
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("user already exists")]
    UserExists,
    #[error("user not found")]
    UserNotFound,
    #[error("token invalid")]
    TokenInvalid,
    #[error("token expired")]
    TokenExpired,
    #[error("captcha error: {0}")]
    Captcha(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("internal error: {0}")]
    InternalError(String),
}

#[derive(Debug, Clone)]
pub struct SignupInput {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct LoginResult {
    pub user_id: UserId,
    pub tokens: AuthTokens,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccessToken(pub String);

#[derive(Debug, Clone, Serialize)]
pub struct RefreshToken(pub String);

#[derive(Debug, Clone, Serialize)]
pub struct AuthTokens {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
    pub access_token_expires_at: DateTime<Utc>,
    pub refresh_token_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TokenVerifyResult {
    pub user_id: UserId,
    pub jti: Option<String>,
}

#[async_trait::async_trait]
pub trait TokenCodec: Send + Sync {
    async fn issue_access_token(
        &self,
        user: UserId,
        jti: Option<String>,
    ) -> Result<(AccessToken, DateTime<Utc>), AuthError>;
    async fn issue_refresh_token(
        &self,
        user: UserId,
        jti: String,
    ) -> Result<(RefreshToken, DateTime<Utc>), AuthError>;
    async fn verify_access_token(
        &self,
        token: &AccessToken,
    ) -> Result<TokenVerifyResult, AuthError>;
    async fn verify_refresh_token(
        &self,
        token: &RefreshToken,
    ) -> Result<TokenVerifyResult, AuthError>;
}

#[async_trait::async_trait]
pub trait CredentialHasher: Send + Sync {
    async fn hash_password(&self, password: &str) -> Result<String, AuthError>;
    async fn verify_password(&self, password: &str, password_hash: &str)
    -> Result<bool, AuthError>;
}

#[async_trait::async_trait]
pub trait AuthService: Send + Sync {
    async fn signup(&self, request: SignupInput) -> Result<UserId, AuthError>;
    async fn login(&self, request: LoginInput) -> Result<LoginResult, AuthError>;
    async fn verify_token(&self, token: &str) -> Result<UserId, AuthError>;
    async fn refresh_token(&self, refresh_token: &str) -> Result<AuthTokens, AuthError>;
}
