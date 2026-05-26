use crate::domain_model::{ConversationId, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// region relationship service
#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct GroupId(pub uuid::Uuid);

#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct GroupInvitationId(pub uuid::Uuid);

impl fmt::Display for GroupInvitationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupInvitationSummary {
    pub invitation_id: GroupInvitationId,
    pub group_id: GroupId,
    pub group_name: String,
    pub inviter_id: UserId,
    pub inviter_username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct GroupInvitationCursor {
    pub created_at: DateTime<Utc>,
    pub invitation_id: GroupInvitationId,
}

impl FromStr for GroupInvitationCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date_str, id_str) = s.split_once('~').ok_or("invalid cursor format")?;
        let created_at = date_str
            .parse::<DateTime<Utc>>()
            .map_err(|e| e.to_string())?;
        let invitation_id = uuid::Uuid::parse_str(id_str)
            .map(GroupInvitationId)
            .map_err(|e| e.to_string())?;
        Ok(GroupInvitationCursor {
            created_at,
            invitation_id,
        })
    }
}

#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct GroupJoinRequestId(pub uuid::Uuid);

impl fmt::Display for GroupJoinRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupJoinRequestSummary {
    pub request_id: GroupJoinRequestId,
    pub group_id: GroupId,
    pub requester_id: UserId,
    pub requester_username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct GroupJoinRequestCursor {
    pub created_at: DateTime<Utc>,
    pub request_id: GroupJoinRequestId,
}

impl FromStr for GroupJoinRequestCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date_str, id_str) = s.split_once('~').ok_or("invalid cursor format")?;
        let created_at = date_str
            .parse::<DateTime<Utc>>()
            .map_err(|e| e.to_string())?;
        let request_id = uuid::Uuid::parse_str(id_str)
            .map(GroupJoinRequestId)
            .map_err(|e| e.to_string())?;
        Ok(GroupJoinRequestCursor {
            created_at,
            request_id,
        })
    }
}

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

impl FromStr for GroupCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date_str, id_str) = s.split_once('~').ok_or("invalid cursor format")?;
        let created_at = date_str
            .parse::<DateTime<Utc>>()
            .map_err(|e| e.to_string())?;
        let group_id = uuid::Uuid::parse_str(id_str)
            .map(GroupId)
            .map_err(|e| e.to_string())?;
        Ok(GroupCursor { created_at, group_id })
    }
}

pub struct MemberCursor {
    pub joined_at: DateTime<Utc>,
    pub user: UserId, // tiebreaker
}

impl FromStr for MemberCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date_str, id_str) = s.split_once('~').ok_or("invalid cursor format")?;
        let joined_at = date_str
            .parse::<DateTime<Utc>>()
            .map_err(|e| e.to_string())?;
        let user = uuid::Uuid::parse_str(id_str)
            .map(UserId)
            .map_err(|e| e.to_string())?;
        Ok(MemberCursor { joined_at, user })
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum GroupMemberRole {
    Owner,
    Member,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupSummary {
    pub group_id: GroupId,
    pub name: String,
    pub my_role: GroupMemberRole, // smell hint: this field seems redundant
    pub conversation_id: ConversationId,
    pub member_count: u32, // smell hint: this field seems redundant
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberSummary {
    pub user_id: UserId,
    pub username: String,
    pub joined_at: DateTime<Utc>,
}
