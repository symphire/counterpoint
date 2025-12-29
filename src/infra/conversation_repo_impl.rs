use chrono::{DateTime, Utc};
use sqlx::MySqlPool;
use crate::domain::{ChatError, ConversationId, ConversationKind, ConversationPeer, GroupId, MessageOffset, PageSize, RecentConversation, RelationError, StorageTx, TimeCursor, UserId};
use crate::infra::{downcast, ConversationRepo};

pub struct MySqlConversationRepo {
    pool: MySqlPool,
}

impl MySqlConversationRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ConversationRepo for MySqlConversationRepo {
    async fn get_conversation_member_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        conversation_id: ConversationId,
    ) -> Result<Vec<UserId>, RelationError> {
        let tx = downcast(tx);

        let rows = sqlx::query_scalar!(
        r#"SELECT user_id AS "user_id: UserId" FROM conversation_member WHERE conversation_id = ?"#,
        conversation_id
    )
            .fetch_all(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("query conversation member: {e}")))?;

        Ok(rows)
    }

    async fn create_direct_conversation_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        a: UserId,
        b: UserId,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError> {
        let tx = downcast(tx);

        sqlx::query("INSERT INTO conversation (conversation_id, kind_id) VALUES (?, 1)")
            .bind(conversation_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("insert direct conversation: {e}")))?;

        sqlx::query(
            "INSERT INTO conversation_counter (conversation_id, next_offset) VALUES (?, 1)",
        )
            .bind(conversation_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("insert conversation_counter: {e}")))?;

        sqlx::query(
            r#"
INSERT INTO conversation_member (conversation_id, user_id)
VALUES (?, ?),
       (?, ?)
"#,
        )
            .bind(conversation_id)
            .bind(a)
            .bind(conversation_id)
            .bind(b)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("insert conversation_member: {e}")))?;

        Ok(())
    }

    async fn create_group_conversation_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError> {
        let tx = downcast(tx);

        sqlx::query("INSERT INTO conversation (conversation_id, kind_id) VALUES (?, 2)")
            .bind(conversation_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("insert group conversation: {e}")))?;

        sqlx::query(
            "INSERT INTO conversation_counter (conversation_id, next_offset) VALUES (?, 1)",
        )
            .bind(conversation_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("insert conversation_counter: {e}")))?;

        Ok(())
    }

    async fn list_for_user_recent_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
        page_size: PageSize,
        after: Option<TimeCursor>,
    ) -> Result<Vec<ConversationId>, ChatError> {
        let tx = downcast(tx);
        let ps = page_size.0 as i64;

        // We only include conversations that have at least one message,
        // because TimeCursor.last_msg_at is non-null.
        let ids: Vec<ConversationId> = if let Some(cur) = after {
            sqlx::query_scalar(
                r#"
SELECT c.conversation_id
FROM conversation_member cm
JOIN conversation c ON c.conversation_id = cm.conversation_id
WHERE cm.user_id = ?
  AND (c.last_msg_at < ? OR (c.last_msg_at = ? AND c.conversation_id < ?))
ORDER BY c.last_msg_at DESC, c.conversation_id DESC
LIMIT ?
"#,
            )
                .bind(user_id)
                .bind(cur.last_msg_at)
                .bind(cur.last_msg_at)
                .bind(cur.conversation_id)
                .bind(ps)
                .fetch_all(tx.conn())
                .await
                .map_err(|e| ChatError::Store(format!("recent(after) ids: {e}")))?
        } else {
            sqlx::query_scalar(
                r#"
SELECT c.conversation_id
FROM conversation_member cm
JOIN conversation c ON c.conversation_id = cm.conversation_id
WHERE cm.user_id = ?
  AND c.last_msg_at IS NOT NULL
ORDER BY c.last_msg_at DESC, c.conversation_id DESC
LIMIT ?
"#,
            )
                .bind(user_id)
                .bind(ps)
                .fetch_all(tx.conn())
                .await
                .map_err(|e| ChatError::Store(format!("recent(first) ids: {e}")))?
        };

        Ok(ids)
    }

    async fn hydrate_conversation_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        user_id: UserId,
        conversation_ids: Vec<ConversationId>,
    ) -> Result<Vec<RecentConversation>, ChatError> {
        #[derive(sqlx::FromRow)]
        struct RecentHydrateRow {
            conversation_id: ConversationId,
            kind_id: u8,
            last_msg_off: u64,
            last_msg_at: Option<DateTime<Utc>>,
            group_id: Option<GroupId>,
            group_name: Option<String>,
            other_user: Option<UserId>,
            other_username: Option<String>,
        }

        let tx = downcast(tx);

        let placeholders = std::iter::repeat("?")
            .take(conversation_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let field_expr = placeholders.clone();

        // Compose query string
        let sql = format!(
            r#"
SELECT
    c.conversation_id,
    c.kind_id,
    c.last_msg_off,
    c.last_msg_at,
    cg.group_id,
    cg.group_name,
    ou.user_id     AS other_user,
    ou.username    AS other_username
FROM conversation AS c
         LEFT JOIN chat_group AS cg
                   ON cg.conversation_id = c.conversation_id
         LEFT JOIN LATERAL (
    SELECT cm.user_id
    FROM conversation_member AS cm
    WHERE cm.conversation_id = c.conversation_id
      AND cm.user_id <> ?
    ORDER BY cm.user_id
    LIMIT 1
    ) AS cu ON TRUE
         LEFT JOIN user AS ou
                   ON ou.user_id = cu.user_id
WHERE c.conversation_id IN ({in_list})
ORDER BY FIELD(c.conversation_id, {field_list})
"#,
            in_list = placeholders,
            field_list = field_expr,
        );

        tracing::trace!("query string in hydrate_conversation_in_tx: {}", sql);

        let mut q = sqlx::query_as::<_, RecentHydrateRow>(&sql).bind(user_id);
        // IN list
        for id in &conversation_ids {
            q = q.bind(*id);
        }
        // FIELD list
        for id in &conversation_ids {
            q = q.bind(*id);
        }

        let rows: Vec<RecentHydrateRow> = q
            .fetch_all(tx.conn())
            .await
            .map_err(|e| ChatError::Store(format!("hydrate recent by kind_id: {e}")))?;

        let out = rows
            .into_iter()
            .map(|r| {
                let peer = match r.kind_id {
                    kind if kind == ConversationKind::Group as u8 => {
                        let (gid, name) = match (r.group_id, r.group_name) {
                            (Some(gid), Some(name)) => (gid, name),
                            _ => {
                                return Err(ChatError::Store(
                                    "group convo missing chat_group row".into(),
                                ))
                            }
                        };
                        ConversationPeer::Group {
                            group_id: gid,
                            name,
                        }
                    }
                    kind if kind == ConversationKind::Direct as u8 => {
                        let (other, name) = match (r.other_user, r.other_username) {
                            (Some(other), Some(name)) => (other, name),
                            _ => {
                                return Err(ChatError::Store(
                                    "direct convo missing other_user".into(),
                                ))
                            }
                        };
                        ConversationPeer::Direct {
                            other_user: other,
                            name,
                        }
                    }
                    _ => return Err(ChatError::Store(format!("unknown kind_id: {}", r.kind_id))),
                };

                Ok(RecentConversation {
                    conversation_id: r.conversation_id,
                    peer,
                    last_msg_off: MessageOffset(r.last_msg_off as u64),
                    last_msg_at: r.last_msg_at,
                })
            })
            .collect::<Result<Vec<_>, ChatError>>()?;

        Ok(out)
    }
}