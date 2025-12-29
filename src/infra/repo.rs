use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::domain::{AuthError, ChatError, ConversationId, FriendCursor, FriendSummary, GroupCursor, GroupId, GroupMemberRole, GroupSummary, IdempotencyKey, MemberCursor, MemberSummary, MessageId, MessageOffset, MessageRecord, OffsetCursor, PageSize, RecentConversation, RelationError, StorageTx, TimeCursor, UserId};

// region auth repo

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

// endregion


// region user repo

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

// endregion


// region friendship repo

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

// endregion


// region group repo

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

// endregion


// region group idempotency repo

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

// endregion


// region conversation repo

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

// endregion


// region conversation role repo

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

// endregion


// region message repo

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

// endregion


// region outbox repo

#[derive(Debug, Serialize, Deserialize)]
pub struct C2SEnvelope {
    pub sender: UserId,
    pub body: C2SCommand,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "lowercase")]
pub enum C2SCommand {
    ChatMessageSend(ChatMessageSend),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessageSend {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct S2CEnvelope {
    pub receivers: Vec<UserId>,
    pub body: S2CEvent,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "lowercase")]
pub enum S2CEvent {
    ChatMessageACK(ChatMessageACK),
    ChatMessageNew(ChatMessageNew),
    FriendshipNew(FriendshipNew),
    GroupNew(GroupNew),
    GroupMemberNew(GroupMemberNew),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessageACK {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub message_offset: MessageOffset,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessageNew {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub message_offset: MessageOffset,
    pub content: String,
    pub sender: UserId,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FriendshipNew {
    pub conversation_id: ConversationId,
    pub other: UserId,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupNew {
    pub conversation_id: ConversationId,
    pub group_id: GroupId,
    pub group_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupMemberNew {
    pub conversation_id: ConversationId,
    pub group_id: GroupId,
    pub member_id: UserId,
    pub username: String,
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct EventId(pub uuid::Uuid);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "chat.message.new")]
    ChatMessageNew,
    #[serde(rename = "friendship.new")]
    FriendshipNew,
    #[serde(rename = "group.new")]
    GroupNew,
    #[serde(rename = "group.member.new")]
    GroupMemberNew,
}

#[derive(Debug, Clone)]
pub struct OutboxEvent {
    pub event_id: EventId,
    pub event_type: EventType,
    pub partition_key: Option<uuid::Uuid>,

    pub receivers_json: serde_json::Value,
    pub payload_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl OutboxEvent {
    pub fn new<T: Serialize>(
        event_type: EventType,
        partition_key: Option<uuid::Uuid>,
        receivers: Vec<UserId>,
        payload: &T,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            event_id: EventId(uuid::Uuid::new_v4()),
            event_type,
            partition_key,
            receivers_json: serde_json::to_value(receivers)?,
            payload_json: serde_json::to_value(payload)?,
            created_at: Utc::now(),
        })
    }
}

#[async_trait::async_trait]
pub trait OutboxRepo: Send + Sync {
    async fn enqueue_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        event: &OutboxEvent,
    ) -> anyhow::Result<()>;

    async fn claim_ready_batch_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        now: DateTime<Utc>,
        limit: u32,
    ) -> anyhow::Result<Vec<OutboxEvent>>;

    async fn mark_delivered_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        event_id: EventId,
        delivered_at: DateTime<Utc>,
    ) -> anyhow::Result<()>;

    async fn reschedule_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        event_id: EventId,
        next_attempt_at: DateTime<Utc>,
        last_error: &str,
    ) -> anyhow::Result<()>;
}

// endregion