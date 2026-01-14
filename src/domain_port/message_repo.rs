use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;

#[async_trait::async_trait]
pub trait MessageRepo: Send + Sync {
    async fn insert_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        conversation_id: ConversationId,
        sender: UserId,
        content: &str,
        message_id: MessageId,
    ) -> Result<MessageRecord, ChatError>;
    async fn list_before_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        conversation_id: ConversationId,
        page_size: PageSize,
        before: Option<OffsetCursor>,
    ) -> Result<Vec<MessageRecord>, ChatError>;
}
