use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct ConversationId(pub uuid::Uuid);

#[repr(u8)]
pub enum ConversationKind {
    Direct = 1,
    Group = 2,
}
