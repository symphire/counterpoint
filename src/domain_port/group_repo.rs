use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;

#[derive(Debug, Clone)]
pub struct GroupShortSummary {
    pub group_id: GroupId,
    pub name: String,
    pub conversation_id: ConversationId,
}

#[async_trait::async_trait]
pub trait GroupRepo: Send + Sync {
    async fn get_group_summary_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        group_id: GroupId,
    ) -> Result<GroupShortSummary, RelationError>;
    async fn insert_chat_group_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        group_id: GroupId,
        owner: UserId,
        name: &str,
        description: Option<&str>,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError>;
    async fn get_conversation_id_by_group(
        &self,
        group_id: GroupId,
    ) -> Result<Option<ConversationId>, RelationError>;
    async fn list_groups(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<GroupCursor>,
    ) -> Result<Vec<GroupSummary>, RelationError>;
    async fn list_group_members_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        group: GroupId,
        page_size: PageSize,
        after: Option<MemberCursor>,
    ) -> Result<Vec<MemberSummary>, RelationError>;
}
