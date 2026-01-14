use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;

#[async_trait::async_trait]
pub trait ConversationRepo: Send + Sync {
    async fn get_conversation_member_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        conversation_id: ConversationId,
    ) -> Result<Vec<UserId>, RelationError>;
    async fn create_direct_conversation_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        a: UserId,
        b: UserId,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError>;
    async fn create_group_conversation_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError>;

    /// Recent for a user, order by (last_msg_at DESC, conversation_id DESC)
    async fn list_for_user_recent_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
        page_size: PageSize,
        after: Option<TimeCursor>,
    ) -> Result<Vec<ConversationId>, ChatError>;
    async fn hydrate_conversation_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
        conversation_ids: Vec<ConversationId>,
    ) -> Result<Vec<RecentConversation>, ChatError>;
}
