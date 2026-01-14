use crate::application_port::{AuthError, UserService};
use crate::domain_model::UserId;
use crate::domain_port::{TxManager, UserRepo};
use std::sync::Arc;

pub struct RealUserService {
    user_repo: Arc<dyn UserRepo>,
    tx_manager: Arc<dyn TxManager>,
}

impl RealUserService {
    pub fn new(user_repo: Arc<dyn UserRepo>, tx_manager: Arc<dyn TxManager>) -> RealUserService {
        RealUserService {
            user_repo,
            tx_manager,
        }
    }
}

#[async_trait::async_trait]
impl UserService for RealUserService {
    async fn resolve_username(&self, username: &str) -> Result<UserId, AuthError> {
        let mut tx = self
            .tx_manager
            .begin()
            .await
            .map_err(|e| AuthError::Store(e.to_string()))?;

        let user_id = self
            .user_repo
            .get_id_by_username_in_tx(&mut *tx, username)
            .await?;

        tx.commit()
            .await
            .map_err(|e| AuthError::Store(e.to_string()))?;

        Ok(user_id)
    }
}
