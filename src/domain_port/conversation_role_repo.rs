use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;

#[async_trait::async_trait]
pub trait ConversationRoleRepo: Send + Sync {
    async fn get_role_by_conversation_id(
        &self,
        user_id: UserId,
        conversation_id: ConversationId,
    ) -> Result<GroupMemberRole, RelationError>;
    async fn ensure_defaults_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError>;
    async fn assign_role_by_name_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        conversation_id: ConversationId,
        user_id: UserId,
        role_name: &str,
    ) -> Result<(), RelationError>;
    async fn membership_exists_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        conversation_id: ConversationId,
        user_id: UserId,
    ) -> Result<bool, RelationError>;
}
