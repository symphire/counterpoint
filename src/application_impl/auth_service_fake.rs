use crate::application_port::*;
use crate::domain_model::UserId;
use chrono::{Duration, Utc};

#[derive(Debug)]
pub struct FakeAuthService;

impl FakeAuthService {
    pub fn new() -> Self {
        Self
    }
}

// Minimal fake implementation for basic use only.
// Extend to simulate more error cases and configurable responses when needed.
#[async_trait::async_trait]
impl AuthService for FakeAuthService {
    async fn signup(&self, request: SignupInput) -> Result<UserId, AuthError> {
        Ok(get_fake_id(&request.username))
    }

    async fn login(&self, request: LoginInput) -> Result<LoginResult, AuthError> {
        Ok(LoginResult {
            user_id: get_fake_id(&request.username),
            tokens: get_fake_token(&request.username),
        })
    }

    async fn verify_token(&self, token: &str) -> Result<UserId, AuthError> {
        if let Some(username) = token.strip_prefix("fake-access-token:") {
            Ok(get_fake_id(&username))
        } else {
            Err(AuthError::TokenInvalid)
        }
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<AuthTokens, AuthError> {
        if let Some(username) = refresh_token.strip_prefix("fake-refresh-token:") {
            Ok(get_fake_token(&username))
        } else {
            Err(AuthError::TokenInvalid)
        }
    }
}

fn get_fake_id(username: &str) -> UserId {
    UserId(uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        username.as_bytes(),
    ))
}

fn get_fake_token(username: &str) -> AuthTokens {
    let now = Utc::now();
    AuthTokens {
        access_token: AccessToken(format!("fake-access-token:{}", username)),
        access_token_expires_at: now + Duration::days(1), // 1 day
        refresh_token: RefreshToken(format!("fake-refresh-token:{}", username)),
        refresh_token_expires_at: now + Duration::days(7), // 7 days
    }
}
