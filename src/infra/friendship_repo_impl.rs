use chrono::{DateTime, Utc};
use sqlx::mysql::MySqlDatabaseError;
use sqlx::{MySqlPool, Row};
use crate::domain::{ConversationId, FriendCursor, FriendSummary, PageSize, RelationError, StorageTx, UserId, UserPair};
use crate::infra::{downcast, is_dup_key, FriendshipIdemClaim, FriendshipRepo};

pub struct MySqlFriendshipRepo {
    pool: MySqlPool,
}

impl MySqlFriendshipRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl FriendshipRepo for MySqlFriendshipRepo {
    async fn claim(
        &self,
        a: UserId,
        b: UserId,
        requested_by: UserId,
    ) -> Result<FriendshipIdemClaim, RelationError> {
        if a == b {
            return Err(RelationError::Store(
                "cannot create direct conversation with self".to_string(),
            ));
        }
        if requested_by != a && requested_by != b {
            return Err(RelationError::Store("bad request".to_string()));
        }

        let pair = UserPair::new(a, b);

        let res = sqlx::query(
            r#"
INSERT INTO friendship (user_min, user_max, status, requested_by)
VALUES (?, ?, 'accepted', ?)
"#,
        )
            .bind(pair.min())
            .bind(pair.max())
            .bind(requested_by)
            .execute(&self.pool)
            .await;

        match res {
            Ok(_) => Ok(FriendshipIdemClaim::Won),
            Err(e) if is_dup_key(&e) => Ok(FriendshipIdemClaim::Existing),
            Err(e) => Err(RelationError::Store(format!("friendship idem insert: {e}"))),
        }
    }

    async fn insert_friendship_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        a: UserId,
        b: UserId,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError> {
        if a == b {
            return Err(RelationError::Store(
                "cannot create direct conversation with self".to_string(),
            ));
        }

        let pair = UserPair::new(a, b);

        let tx = downcast(tx);

        sqlx::query(
            "INSERT INTO direct_pair (user_min, user_max, conversation_id) VALUES (?, ?, ?)",
        )
            .bind(pair.min())
            .bind(pair.max())
            .bind(conversation_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("insert friendship conversation: {e}")))?;

        Ok(())
    }

    async fn get_conversation_id_by_friendship(
        &self,
        a: UserId,
        b: UserId,
    ) -> Result<ConversationId, RelationError> {
        let row =
            sqlx::query("SELECT conversation_id FROM direct_pair WHERE user_min=? AND user_max=?")
                .bind(a)
                .bind(b)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RelationError::Store(format!("select direct conversation: {e}")))?;

        let conv_id = row
            .try_get::<ConversationId, _>("conversation_id")
            .map_err(|e| RelationError::Store(format!("decode conversation_id: {e}")))?;

        Ok(conv_id)
    }

    async fn list_friends_with_conversations(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<FriendCursor>,
    ) -> Result<Vec<FriendSummary>, RelationError> {
        // Mapped from SQL
        struct Row {
            other_user: UserId,
            username: String,
            conversation_id: ConversationId,
            since: DateTime<Utc>,
        }

        // Without cursor
        if after.is_none() {
            let rows = sqlx::query_as!(
                Row,
                r#"
SELECT
    IF(? = f.user_min, f.user_max, f.user_min) AS "other_user: UserId",
    u.username                                 AS username,
    dp.conversation_id                         AS "conversation_id: ConversationId",
    f.created_at                               AS "since: DateTime<Utc>"
FROM friendship f
JOIN direct_pair dp
  ON dp.user_min = f.user_min AND dp.user_max = f.user_max
JOIN user u
  ON u.user_id = IF(? = f.user_min, f.user_max, f.user_min)
WHERE f.status = 'accepted'
  AND (? = f.user_min OR ? = f.user_max)
ORDER BY f.created_at DESC,
         u.username ASC
LIMIT ?
"#,
                user_id,
                user_id,
                user_id,
                user_id,
                page_size.0 as i64
            )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RelationError::Store(format!("list friends (no cursor): {e}")))?;

            let out = rows
                .into_iter()
                .map(|r| FriendSummary {
                    user_id: r.other_user,
                    username: r.username,
                    conversation_id: r.conversation_id,
                    since: r.since,
                })
                .collect();

            return Ok(out);
        }

        // With cursor
        let cur = after.unwrap();

        let rows = sqlx::query_as!(
            Row,
            r#"
SELECT
    IF(? = f.user_min, f.user_max, f.user_min) AS "other_user: UserId",
    u.username                                 AS username,
    dp.conversation_id                         AS "conversation_id: ConversationId",
    f.created_at                               AS "since: DateTime<Utc>"
FROM friendship f
JOIN direct_pair dp
  ON dp.user_min = f.user_min AND dp.user_max = f.user_max
JOIN user u
ON u.user_id = IF(? = f.user_min, f.user_max, f.user_min)
WHERE f.status = 'accepted'
  AND (? = f.user_min OR ? = f.user_max)
  AND (
      f.created_at < ?
      OR (f.created_at = ? AND IF(? = f.user_min, f.user_max, f.user_min) < ?)
  )
ORDER BY f.created_at DESC,
         u.username ASC
LIMIT ?
"#,
            user_id,
            user_id,
            user_id,
            cur.since,
            cur.since,
            user_id,
            cur.other_user,
            user_id,
            page_size.0 as i64
        )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelationError::Store(format!("list friends (with cursor): {e}")))?;

        let out = rows
            .into_iter()
            .map(|r| FriendSummary {
                user_id: r.other_user,
                username: r.username,
                conversation_id: r.conversation_id,
                since: r.since,
            })
            .collect();

        Ok(out)
    }
}