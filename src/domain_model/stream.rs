use crate::domain_model::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
