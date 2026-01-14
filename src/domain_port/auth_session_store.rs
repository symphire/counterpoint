use crate::application_port::*;
use crate::domain_model::*;

#[async_trait::async_trait]
pub trait AuthSessionStore: Send + Sync {
    /// Save a refresh token jti for a user with TTL.
    async fn save_refresh_jti(
        &self,
        user_id: UserId,
        jti: &str,
        ttl_secs: u64,
    ) -> Result<(), AuthError>;
    /// Check if JTI is present (valid). If valid and consume=true, delete it (rotation).
    async fn check_refresh_jti(
        &self,
        user_id: UserId,
        jti: &str,
        consume: bool,
    ) -> Result<Option<UserId>, AuthError>;
}
