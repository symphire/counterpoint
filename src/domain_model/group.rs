use crate::domain_model::{ConversationId, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

// region relationship service
#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct GroupId(pub uuid::Uuid);

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct GroupCursor {
    pub created_at: DateTime<Utc>,
    pub group_id: GroupId, // tiebreaker
}

pub struct MemberCursor {
    pub joined_at: DateTime<Utc>,
    pub user: UserId, // tiebreaker
}

#[derive(Debug, Clone)]
pub enum GroupMemberRole {
    Owner,
    Member,
}

#[derive(Debug, Clone)]
pub struct GroupSummary {
    pub group_id: GroupId,
    pub name: String,
    pub my_role: GroupMemberRole, // smell hint: this field seems redundant
    pub conversation_id: ConversationId,
    pub member_count: u32, // smell hint: this field seems redundant
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MemberSummary {
    pub user_id: UserId,
    pub username: String,
    pub joined_at: DateTime<Utc>,
}
