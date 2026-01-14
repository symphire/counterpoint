use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub user_id: UserId,
    pub username: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait UserRepo: Send + Sync {
    async fn create_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
        username: &str,
    ) -> Result<(), AuthError>;

    async fn get_username_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
    ) -> Result<String, AuthError>;

    async fn get_id_by_username_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        username: &str,
    ) -> Result<UserId, AuthError>;

    async fn username_exists(&self, username: &str) -> Result<bool, AuthError>;

    async fn id_exists(&self, user_id: UserId) -> Result<bool, AuthError>;
}
