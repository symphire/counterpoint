use crate::application_port::AuthError;
use crate::domain_model::UserId;

#[async_trait::async_trait]
pub trait UserService: Send + Sync {
    async fn resolve_username(&self, username: &str) -> Result<UserId, AuthError>;
}
