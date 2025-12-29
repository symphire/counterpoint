use crate::domain::{
    ChatError, ConversationId, MessageId, MessageOffset, MessageRecord, OffsetCursor, PageSize,
    StorageTx, UserId,
};
use crate::infra::{downcast, is_dup_key, MessageRepo};
use chrono::{DateTime, Utc};
use sqlx::MySqlPool;

#[derive(sqlx::FromRow)]
struct MessageRow {
    message_id: MessageId,
    conversation_id: ConversationId,
    message_offset: u64,
    sender_id: UserId,
    content: String,
    created_at: DateTime<Utc>,
}

pub struct MySqlMessageRepo {
    pool: MySqlPool,
}

impl MySqlMessageRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl MessageRepo for MySqlMessageRepo {
    async fn insert_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        conversation_id: ConversationId,
        sender: UserId,
        content: &str,
        message_id: MessageId,
    ) -> Result<MessageRecord, ChatError> {
        let mut tx = downcast(tx);

        // 1) Grab the next offset
        let res = sqlx::query!(
            r#"
INSERT INTO conversation_counter (conversation_id, next_offset)
VALUES (?, 1)
ON DUPLICATE KEY UPDATE next_offset = LAST_INSERT_ID(next_offset + 1)
"#,
            conversation_id,
        )
        .execute(tx.conn())
        .await
        .map_err(|e| ChatError::Store(format!("update counter: {e}")))?;

        let assigned_off = MessageOffset(res.last_insert_id());

        // 2) Insert message row
        let insert_res = sqlx::query!(
            r#"
INSERT INTO message (message_id, conversation_id, message_offset, sender_id, content)
VALUES (?, ?, ?, ?, ?)
"#,
            message_id,
            conversation_id,
            assigned_off,
            sender,
            content
        )
        .execute(tx.conn())
        .await;

        // 3) Handle idempotency
        let row: MessageRow = match insert_res {
            Ok(_) => {
                // Insert succeeded
                sqlx::query_as!(
                    MessageRow,
                    r#"
SELECT message_id AS "message_id: MessageId",
       conversation_id AS "conversation_id: ConversationId",
       message_offset,
       sender_id AS "sender_id: UserId",
       content,
       created_at AS "created_at: DateTime<Utc>"
FROM message
WHERE message_id = ?
"#,
                    message_id,
                )
                .fetch_one(tx.conn())
                .await
                .map_err(|e| ChatError::Store(format!("fetch inserted message: {e}")))?
            }
            Err(e) if is_dup_key(&e) => {
                // Existing message
                sqlx::query_as!(
                    MessageRow,
                    r#"
SELECT message_id AS "message_id: MessageId",
       conversation_id AS "conversation_id: ConversationId",
       message_offset,
       sender_id AS "sender_id: UserId",
       content,
       created_at AS "created_at: DateTime<Utc>"
FROM message
WHERE message_id = ?
"#,
                    message_id,
                )
                .fetch_one(tx.conn())
                .await
                .map_err(|e| ChatError::Store(format!("fetch inserted message: {e}")))?
            }
            Err(e) => return Err(ChatError::Store(format!("insert into message: {e}"))),
        };

        // 4) Advance conversation last pointers
        // NOTE: last_msg_at can be NULL
        sqlx::query!(
            r#"
UPDATE conversation
SET last_msg_off = GREATEST(last_msg_off, ?),
    last_msg_at  = GREATEST(COALESCE(last_msg_at, TIMESTAMP('1970-01-01 00:00:00')), ?)
WHERE conversation_id = ?
"#,
            row.message_offset,
            row.created_at,
            row.conversation_id,
        )
        .execute(tx.conn())
        .await
        .map_err(|e| ChatError::Store(format!("advance conversation last: {e}")))?;

        Ok(MessageRecord {
            message_id: row.message_id,
            conversation_id: row.conversation_id,
            message_offset: MessageOffset(row.message_offset),
            sender: row.sender_id,
            content: row.content,
            created_at: row.created_at,
        })
    }

    async fn list_before_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        conversation_id: ConversationId,
        page_size: PageSize,
        before: Option<OffsetCursor>,
    ) -> Result<Vec<MessageRecord>, ChatError> {
        let tx = downcast(tx);
        let ps = page_size.0 as i64;

        tracing::trace!(
            "list_before_in_tx: conversation_id: {}",
            conversation_id.0.to_string()
        );

        let rows: Vec<MessageRow> = if let Some(before) = before {
            let off = before.offset.0 as i64;
            
            let mut v = sqlx::query_as!(
            MessageRow,
            r#"
SELECT message_id AS "message_id: MessageId",
       conversation_id AS "conversation_id: ConversationId",
       message_offset,
       sender_id AS "sender_id: UserId",
       content,
       created_at AS "created_at: DateTime<Utc>"
FROM message
WHERE conversation_id = ?
  AND message_offset < ?
ORDER BY message_offset DESC
LIMIT ?
"#,
            conversation_id,
            off,
            ps,
        )
                .fetch_all(tx.conn())
                .await
                .map_err(|e| ChatError::Store(format!("list_before_in_tx(before): {e}")))?;

            v.reverse();
            v
        } else {
            let mut v = sqlx::query_as!(
            MessageRow,
            r#"
SELECT message_id AS "message_id: MessageId",
       conversation_id AS "conversation_id: ConversationId",
       message_offset,
       sender_id AS "sender_id: UserId",
       content,
       created_at AS "created_at: DateTime<Utc>"
FROM message
WHERE conversation_id = ?
ORDER BY message_offset DESC
LIMIT ?
"#,
            conversation_id,
            ps,
        )
                .fetch_all(tx.conn())
                .await
                .map_err(|e| ChatError::Store(format!("list_before_in_tx(latest): {e}")))?;

            v.reverse();
            v
        };

        let out = rows
            .into_iter()
            .map(|r| MessageRecord {
                message_id: r.message_id,
                conversation_id: r.conversation_id,
                message_offset: MessageOffset(r.message_offset),
                sender: r.sender_id,
                content: r.content,
                created_at: r.created_at,
            })
            .collect();

        Ok(out)
    }
}
