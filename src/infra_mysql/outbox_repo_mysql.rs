use super::util::downcast;
use crate::domain_port::*;
use chrono::{DateTime, Utc};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::mysql::MySqlRow;
use sqlx::types::JsonValue;
use sqlx::{Database, Decode, Encode, MySqlPool, Row, Type};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EventType::ChatMessageNew => "chat.message.new",
            EventType::FriendshipNew => "friendship.new",
            EventType::GroupNew => "group.new",
            EventType::GroupMemberNew => "group.member.new",
        };
        f.write_str(s)
    }
}

impl FromStr for EventType {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "chat.message.new" => Ok(Self::ChatMessageNew),
            "friendship.new" => Ok(Self::FriendshipNew),
            "group.new" => Ok(Self::GroupNew),
            "group.member.new" => Ok(Self::GroupMemberNew),
            _ => anyhow::bail!("unknown event type: {}", s),
        }
    }
}

impl<'r, DB: Database> Decode<'r, DB> for EventType
where
    &'r str: Decode<'r, DB>,
{
    fn decode(value: <DB as Database>::ValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as Decode<DB>>::decode(value)?;
        Ok(s.parse()?)
    }
}

impl<'q, DB: Database> Encode<'q, DB> for EventType
where
    String: Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as Database>::ArgumentBuffer<'q>,
    ) -> Result<IsNull, BoxDynError> {
        self.to_string().encode_by_ref(buf)
    }
}

impl<DB: Database> Type<DB> for EventType
where
    String: Type<DB>,
{
    fn type_info() -> <DB as Database>::TypeInfo {
        <String as Type<DB>>::type_info()
    }
}

pub struct MySqlOutboxRepo {
    pool: MySqlPool,
}

impl MySqlOutboxRepo {
    pub fn new(pool: MySqlPool) -> Self {
        MySqlOutboxRepo { pool }
    }

    fn row_to_item(r: &MySqlRow) -> OutboxEvent {
        let event_id = r.get::<EventId, _>("event_id");
        let event_type_str = r.get::<&str, _>("event_type");
        let event_type = EventType::from_str(event_type_str).unwrap();
        let partition_key = r.get::<Option<Uuid>, _>("partition_key");

        let receivers_json: JsonValue = r.get("receivers_json");
        let payload_json: JsonValue = r.get("payload_json");

        let created_at = r.get::<DateTime<Utc>, _>("created_at");

        OutboxEvent {
            event_id,
            event_type,
            partition_key,
            receivers_json,
            payload_json,
            created_at,
        }
    }
}

#[async_trait::async_trait]
impl OutboxRepo for MySqlOutboxRepo {
    async fn enqueue_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        event: &OutboxEvent,
    ) -> anyhow::Result<()> {
        let tx = downcast(tx);

        sqlx::query(
            r#"
INSERT INTO outbox (event_id, event_type, partition_key, receivers_json, payload_json)
VALUES (?, ?, ?, ?, ?)
ON DUPLICATE KEY UPDATE event_id = event_id
"#,
        )
        .bind(event.event_id)
        .bind(event.event_type)
        .bind(event.partition_key)
        .bind(&event.receivers_json)
        .bind(&event.payload_json)
        .execute(tx.conn())
        .await?;

        Ok(())
    }

    async fn claim_ready_batch_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        now: DateTime<Utc>,
        limit: u32,
    ) -> anyhow::Result<Vec<OutboxEvent>> {
        let tx = downcast(tx);

        let rows = sqlx::query(
            r#"
SELECT event_id, event_type, partition_key, receivers_json, payload_json, created_at
FROM outbox
WHERE delivered_at IS NULL
  AND next_attempt_at <= ?
ORDER BY created_at ASC
LIMIT ?
FOR UPDATE SKIP LOCKED
"#,
        )
        .bind(now)
        .bind(limit as i64)
        .fetch_all(tx.conn())
        .await?;

        let items = rows.into_iter().map(|r| Self::row_to_item(&r)).collect();
        Ok(items)
    }

    async fn mark_delivered_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        event_id: EventId,
        delivered_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let tx = downcast(tx);

        sqlx::query(
            r#"
UPDATE outbox
SET delivered_at = ?, last_error = NULL
WHERE event_id = ?
"#,
        )
        .bind(delivered_at)
        .bind(event_id)
        .execute(tx.conn())
        .await?;

        Ok(())
    }

    async fn reschedule_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        event_id: EventId,
        next_attempt_at: DateTime<Utc>,
        last_error: &str,
    ) -> anyhow::Result<()> {
        let tx = downcast(tx);

        sqlx::query(
            r#"
UPDATE outbox
SET attempt_count = attempt_count + 1,
    next_attempt_at = ?,
    last_error = LEFT(?, 1024)
WHERE event_id = ?
"#,
        )
        .bind(next_attempt_at)
        .bind(last_error)
        .bind(event_id)
        .execute(tx.conn())
        .await?;

        Ok(())
    }
}
