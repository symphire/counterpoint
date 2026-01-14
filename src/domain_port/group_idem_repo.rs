use crate::application_port::*;
use crate::domain_model::*;

pub enum GroupIdemClaim {
    Won {
        group_id: GroupId,
    },
    Existing {
        group_id: GroupId,
        status: GroupIdemStatus,
        conversation_id: Option<ConversationId>,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum GroupIdemStatus {
    Pending,
    Succeeded,
    Failed,
}

#[async_trait::async_trait]
pub trait GroupIdemRepo: Send + Sync {
    async fn claim(
        &self,
        owner: UserId,
        key: IdempotencyKey,
        proposed_group: GroupId,
    ) -> Result<GroupIdemClaim, RelationError>;
    async fn mark_succeeded(
        &self,
        owner: UserId,
        key: IdempotencyKey,
        group_id: GroupId,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError>;
    async fn mark_failed(
        &self,
        owner: UserId,
        key: IdempotencyKey,
        group_id: GroupId,
        _err: &str,
    ) -> Result<(), RelationError>;
}
