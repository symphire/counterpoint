use serde::{Deserialize, Serialize};
use std::fmt;

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
