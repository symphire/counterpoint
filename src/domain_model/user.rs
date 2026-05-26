use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct UserId(pub uuid::Uuid);

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for UserId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::from_str(s).map(UserId)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserProfile {
    pub user_id: UserId,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct UserCursor {
    pub created_at: DateTime<Utc>,
    pub user_id: UserId,
}

impl FromStr for UserCursor {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date_str, id_str) = s.split_once('~').ok_or("invalid cursor format")?;
        let created_at = date_str
            .parse::<DateTime<Utc>>()
            .map_err(|e| e.to_string())?;
        let user_id = uuid::Uuid::parse_str(id_str)
            .map(UserId)
            .map_err(|e| e.to_string())?;
        Ok(UserCursor { created_at, user_id })
    }
}

pub struct UserPair(UserId, UserId);

impl UserPair {
    pub fn new(a: UserId, b: UserId) -> Self {
        if a < b { Self(a, b) } else { Self(b, a) }
    }

    pub fn min(&self) -> UserId {
        self.0
    }

    pub fn max(&self) -> UserId {
        self.1
    }
}
