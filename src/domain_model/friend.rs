use crate::domain_model::{ConversationId, UserId};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FriendCursor {
    pub since: DateTime<Utc>,
    pub other_user: UserId, // tiebreaker
}

impl FromStr for FriendCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date_str, user_str) = s.split_once('~').ok_or("invalid cursor format")?;

        let since = date_str
            .parse::<DateTime<Utc>>()
            .map_err(|e| e.to_string())?;

        let other_user = uuid::Uuid::parse_str(user_str)
            .map(UserId)
            .map_err(|e| e.to_string())?;

        Ok(FriendCursor { since, other_user })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FriendSummary {
    pub user_id: UserId,
    pub username: String,
    pub conversation_id: ConversationId,
    pub since: DateTime<Utc>,
}
