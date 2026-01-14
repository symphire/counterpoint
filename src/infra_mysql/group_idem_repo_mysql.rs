use super::util::is_dup_key;
use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::*;
use sqlx::{MySqlPool, Row};

pub struct MySqlGroupIdemRepo {
    pool: MySqlPool,
}

impl MySqlGroupIdemRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GroupIdemRepo for MySqlGroupIdemRepo {
    async fn claim(
        &self,
        owner: UserId,
        key: IdempotencyKey,
        proposed_group: GroupId,
    ) -> Result<GroupIdemClaim, RelationError> {
        let res = sqlx::query(
            r#"
INSERT INTO group_create_idem (owner_id, idem_key, proposed_group, status)
VALUES (?, ?, ?, 'pending')
"#,
        )
        .bind(owner)
        .bind(key)
        .bind(proposed_group)
        .execute(&self.pool)
        .await;

        match res {
            Ok(_) => Ok(GroupIdemClaim::Won {
                group_id: proposed_group,
            }),
            Err(e) if is_dup_key(&e) => {
                let row = sqlx::query(
                    r#"
SELECT proposed_group, status, conversation_id FROM group_create_idem
WHERE owner_id=? AND idem_key=?
"#,
                )
                .bind(owner)
                .bind(key)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RelationError::Store(format!("idem select: {e}")))?;

                let gid = row
                    .try_get::<GroupId, _>("group_id")
                    .map_err(|e| RelationError::Store(format!("uuid decode: {e}")))?;

                let status = match row
                    .try_get::<&str, _>("status")
                    .map_err(|e| RelationError::Store(format!("status decode: {e}")))?
                {
                    "pending" => GroupIdemStatus::Pending,
                    "succeeded" => GroupIdemStatus::Succeeded,
                    "failed" => GroupIdemStatus::Failed,
                    other => return Err(RelationError::Store(format!("idem bad status: {other}"))),
                };

                let conversation_id = row
                    .try_get::<Option<ConversationId>, _>("conversation_id")
                    .map_err(|e| RelationError::Store(format!("uuid decode: {e}")))?;

                Ok(GroupIdemClaim::Existing {
                    group_id: gid,
                    status,
                    conversation_id,
                })
            }
            Err(e) => Err(RelationError::Store(format!("group idem insert: {e}"))),
        }
    }

    async fn mark_succeeded(
        &self,
        owner: UserId,
        key: IdempotencyKey,
        group_id: GroupId,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError> {
        sqlx::query(
            r#"
UPDATE group_create_idem SET status='succeeded'
WHERE owner_id=? AND idem_key=? AND proposed_group=? AND conversation_id=?
"#,
        )
        .bind(owner)
        .bind(key)
        .bind(group_id)
        .bind(conversation_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("idem mark_succeeded: {e}")))?;

        Ok(())
    }

    async fn mark_failed(
        &self,
        owner: UserId,
        key: IdempotencyKey,
        group_id: GroupId,
        _err: &str,
    ) -> Result<(), RelationError> {
        sqlx::query(
            r#"
UPDATE group_create_idem SET status='failed'
WHERE owner_id=? AND idem_key=? AND proposed_group=?
"#,
        )
        .bind(owner)
        .bind(key)
        .bind(group_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("idem mark_failed: {e}")))?;

        Ok(())
    }
}
