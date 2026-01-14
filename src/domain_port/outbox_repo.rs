use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, sqlx::Type,
)]
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
