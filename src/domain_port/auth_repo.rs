use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct AuthCredentialsRecord {
    pub user_id: UserId,
    pub username: String,
    pub password_hash: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait AuthRepo: Send + Sync {
    /// Insert a row. The `user_id` row must already exist (FK).
    async fn create_credentials_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
        username: &str,
        password_hash: &str,
    ) -> Result<(), AuthError>;

    /// Fetch credentials by username (for login).
    async fn get_by_username(
        &self,
        username: &str,
    ) -> Result<Option<AuthCredentialsRecord>, AuthError>;
}
