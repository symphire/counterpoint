use serde::Deserialize;

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct PageSize(pub u16);
