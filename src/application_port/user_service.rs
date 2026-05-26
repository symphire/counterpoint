use crate::application_port::AuthError;
use crate::domain_model::{PageSize, UserId, UserCursor, UserProfile};

#[async_trait::async_trait]
pub trait UserService: Send + Sync {
    async fn resolve_username(&self, username: &str) -> Result<UserId, AuthError>;
    async fn get_profile(&self, user_id: UserId) -> Result<UserProfile, AuthError>;
    async fn search_users(
        &self,
        query: &str,
        page_size: PageSize,
        after: Option<UserCursor>,
    ) -> Result<Vec<UserProfile>, AuthError>;
}
