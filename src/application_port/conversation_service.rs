use crate::domain_model::*;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub enum ConversationPeer {
    Direct { other_user: UserId, name: String },
    Group { group_id: GroupId, name: String },
}

#[derive(Debug, Clone)]
pub struct RecentConversation {
    pub conversation_id: ConversationId,
    pub peer: ConversationPeer,
    pub last_msg_off: MessageOffset,
    pub last_msg_at: Option<DateTime<Utc>>, // NULL before first message
}

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("conversation not found")]
    ConversationNotFound,
    #[error("user not a member of conversation")]
    NotMember,
    #[error("permission denied: {0}")]
    Forbidden(&'static str),
    #[error("idempotency conflict")]
    IdempotentConflict,
    #[error("invalid cursor")]
    BadCursor,
    #[error("conflict: direct conversation already exists")]
    AlreadyExists,
    #[error("store error: {0}")]
    Store(String),
}

#[async_trait::async_trait]
pub trait ConversationService: Send + Sync {
    async fn send_message(
        &self,
        conversation_id: ConversationId,
        sender: UserId,
        content: &str,
        message_id: MessageId,
    ) -> Result<MessageRecord, ChatError>;
    async fn get_history(
        &self,
        user_id: UserId,
        conversation_id: ConversationId,
        page_size: PageSize,
        before: Option<OffsetCursor>,
    ) -> Result<Vec<MessageRecord>, ChatError>;
    async fn recent_conversations(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<TimeCursor>,
    ) -> Result<Vec<RecentConversation>, ChatError>;
}
