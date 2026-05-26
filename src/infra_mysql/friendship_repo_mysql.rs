use super::util::{downcast, is_dup_key};
use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::*;
use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row};

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

    async fn insert_pending(
        &self,
        pair: UserPair,
        requester: UserId,
    ) -> Result<(), RelationError> {
        let res = sqlx::query(
            r#"
INSERT INTO friendship (user_min, user_max, status, requested_by, created_at)
VALUES (?, ?, 'pending', ?, NOW(6))
"#,
        )
        .bind(pair.min())
        .bind(pair.max())
        .bind(requester)
        .execute(&self.pool)
        .await;

        match res {
            Ok(_) => Ok(()),
            Err(e) if is_dup_key(&e) => {
                // Distinguish between already-friends and already-requested
                let row = sqlx::query(
                    "SELECT status FROM friendship WHERE user_min = ? AND user_max = ?",
                )
                .bind(pair.min())
                .bind(pair.max())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| RelationError::Store(e.to_string()))?;

                match row.as_ref().map(|r| r.get::<String, _>("status")).as_deref() {
                    Some("accepted") => Err(RelationError::AlreadyFriends),
                    _ => Err(RelationError::FriendRequestExists),
                }
            }
            Err(e) => Err(RelationError::Store(format!("insert pending friendship: {e}"))),
        }
    }

    async fn accept_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        pair: UserPair,
    ) -> Result<(), RelationError> {
        let tx = downcast(tx);

        let result = sqlx::query(
            "UPDATE friendship SET status = 'accepted' WHERE user_min = ? AND user_max = ? AND status = 'pending'",
        )
        .bind(pair.min())
        .bind(pair.max())
        .execute(tx.conn())
        .await
        .map_err(|e| RelationError::Store(format!("accept friendship: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(RelationError::FriendRequestNotFound);
        }

        Ok(())
    }

    async fn reject(&self, pair: UserPair, recipient: UserId) -> Result<(), RelationError> {
        let result = sqlx::query(
            "DELETE FROM friendship WHERE user_min = ? AND user_max = ? AND status = 'pending' AND requested_by != ?",
        )
        .bind(pair.min())
        .bind(pair.max())
        .bind(recipient)
        .execute(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("reject friendship: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(RelationError::FriendRequestNotFound);
        }

        Ok(())
    }

    async fn remove(&self, pair: UserPair) -> Result<(), RelationError> {
        sqlx::query(
            "DELETE FROM friendship WHERE user_min = ? AND user_max = ?",
        )
        .bind(pair.min())
        .bind(pair.max())
        .execute(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("remove friendship: {e}")))?;

        Ok(())
    }

    async fn list_incoming_requests(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<FriendRequestCursor>,
    ) -> Result<Vec<FriendRequestSummary>, RelationError> {
        let limit = page_size.0 as i64;

        let rows = if let Some(cur) = after {
            sqlx::query(
                r#"
SELECT f.requested_by, u.username, f.created_at
FROM friendship f
JOIN user u ON u.user_id = f.requested_by
WHERE f.status = 'pending'
  AND (f.user_min = ? OR f.user_max = ?)
  AND f.requested_by != ?
  AND (f.created_at > ? OR (f.created_at = ? AND f.requested_by > ?))
ORDER BY f.created_at ASC, f.requested_by ASC
LIMIT ?
"#,
            )
            .bind(user_id)
            .bind(user_id)
            .bind(user_id)
            .bind(cur.requested_at)
            .bind(cur.requested_at)
            .bind(cur.requester_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelationError::Store(format!("list incoming requests: {e}")))?
        } else {
            sqlx::query(
                r#"
SELECT f.requested_by, u.username, f.created_at
FROM friendship f
JOIN user u ON u.user_id = f.requested_by
WHERE f.status = 'pending'
  AND (f.user_min = ? OR f.user_max = ?)
  AND f.requested_by != ?
ORDER BY f.created_at ASC, f.requested_by ASC
LIMIT ?
"#,
            )
            .bind(user_id)
            .bind(user_id)
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelationError::Store(format!("list incoming requests: {e}")))?
        };

        let summaries = rows
            .iter()
            .map(|r| FriendRequestSummary {
                requester_id: r.get::<UserId, _>("requested_by"),
                username: r.get::<String, _>("username"),
                requested_at: r.get::<DateTime<Utc>, _>("created_at"),
            })
            .collect();

        Ok(summaries)
    }
}
