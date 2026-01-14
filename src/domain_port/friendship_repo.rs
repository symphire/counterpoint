use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;

pub enum FriendshipIdemClaim {
    Won,
    Existing,
}

#[async_trait::async_trait]
pub trait FriendshipRepo: Send + Sync {
    async fn claim(
        &self,
        a: UserId,
        b: UserId,
        requested_by: UserId,
    ) -> Result<FriendshipIdemClaim, RelationError>;
    async fn insert_friendship_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        a: UserId,
        b: UserId,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError>;
    async fn get_conversation_id_by_friendship(
        &self,
        a: UserId,
        b: UserId,
    ) -> Result<ConversationId, RelationError>;
    async fn list_friends_with_conversations(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<FriendCursor>,
    ) -> Result<Vec<FriendSummary>, RelationError>;
}
