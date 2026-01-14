use crate::domain_model::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct MessageId(pub uuid::Uuid);

#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct MessageOffset(pub u64);

impl FromStr for MessageOffset {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let offset = s.parse::<u64>().map_err(|e| e.to_string())?;
        Ok(Self(offset))
    }
}

/// Cursor for time-ordered lists (recent convos)
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TimeCursor {
    pub last_msg_at: DateTime<Utc>,
    pub conversation_id: ConversationId, // tie-breaker for stable pagination
}

/// Cursor for offset-ordered lists (history)
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct OffsetCursor {
    pub offset: MessageOffset,
}

impl FromStr for OffsetCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let offset = s
            .parse::<MessageOffset>()
            .map_err(|e| format!("invalid offset: {}", e))?;

        Ok(OffsetCursor { offset })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageRecord {
    pub message_id: MessageId,
    pub conversation_id: ConversationId,
    pub message_offset: MessageOffset,
    pub sender: UserId,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
