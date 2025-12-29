use sqlx::{MySqlPool, Row};
use crate::domain::{ConversationId, GroupMemberRole, RelationError, StorageTx, UserId};
use crate::infra::{downcast, ConversationRoleRepo};

pub struct MySqlConversationRoleRepo {
    pool: MySqlPool,
}

impl MySqlConversationRoleRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ConversationRoleRepo for MySqlConversationRoleRepo {
    async fn get_role_by_conversation_id(
        &self,
        user_id: UserId,
        conversation_id: ConversationId,
    ) -> Result<GroupMemberRole, RelationError> {
        let row = sqlx::query(
            r#"
SELECT name
FROM conversation_role r
JOIN conversation_member_role m
  ON m.role_id = r.role_id
WHERE m.user_id = ? AND m.conversation_id = ?
"#,
        )
            .bind(user_id)
            .bind(conversation_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RelationError::NotMember)?;

        let role_str: &str = row
            .try_get("name")
            .map_err(|e| RelationError::Store(format!("decode role name: {e}")))?;
        let role = match role_str {
            "owner" => GroupMemberRole::Owner,
            "member" => GroupMemberRole::Member,
            r => return Err(RelationError::Store(format!("bad role name: {r}"))),
        };

        Ok(role)
    }

    async fn ensure_defaults_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError> {
        let tx = downcast(tx);

        // 1) Upsert owner role
        sqlx::query(
            r#"
INSERT INTO conversation_role (conversation_id, name)
VALUES (?, 'owner')
ON DUPLICATE KEY UPDATE name = name
"#,
        )
            .bind(conversation_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("upsert owner role: {e}")))?;

        // 2) Upsert member role
        sqlx::query(
            r#"
INSERT INTO conversation_role (conversation_id, name)
VALUES (?, 'member')
ON DUPLICATE KEY UPDATE name = name
"#,
        )
            .bind(conversation_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("upsert member role: {e}")))?;

        // 3) Fetch role_ids
        let row = sqlx::query(
            r#"
SELECT
    MAX(CASE WHEN name='owner'  THEN role_id END) AS owner_role_id,
    MAX(CASE WHEN name='member' THEN role_id END) AS member_role_id
FROM conversation_role
WHERE conversation_id = ?
"#,
        )
            .bind(conversation_id)
            .fetch_one(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("select role ids: {e}")))?;

        let owner_role_id = row
            .try_get::<i64, _>("owner_role_id")
            .map_err(|e| RelationError::Store(format!("i64 role decode: {e}")))?;
        let member_role_id = row
            .try_get::<i64, _>("member_role_id")
            .map_err(|e| RelationError::Store(format!("i64 role decode: {e}")))?;

        // 4) Seed permissions.
        // owner: allow both 'message.send' and 'member.invite'
        sqlx::query(
            r#"
INSERT INTO conversation_role_perm (role_id, perm_id, effect)
SELECT ?, p.perm_id, 'allow' FROM permission p WHERE p.perm_key IN ('message.send', 'member.invite')
ON DUPLICATE KEY UPDATE effect = VALUES(effect)
"#,
        )
            .bind(owner_role_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("seed owner perms: {e}")))?;

        // member: allow 'message.send' only
        sqlx::query(
            r#"
INSERT INTO conversation_role_perm (role_id, perm_id, effect)
SELECT ?, p.perm_id, 'allow' FROM permission p WHERE p.perm_key IN ('message.send')
ON DUPLICATE KEY UPDATE effect = VALUES(effect)
"#,
        )
            .bind(member_role_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("seed member perms: {e}")))?;

        Ok(())
    }

    async fn assign_role_by_name_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        conversation_id: ConversationId,
        user_id: UserId,
        role_name: &str,
    ) -> Result<(), RelationError> {
        let tx = downcast(tx);

        // 1) Resolve role_id by (conversation_id, name)
        let row = sqlx::query(
            "SELECT role_id FROM conversation_role WHERE conversation_id = ? AND name = ?",
        )
            .bind(conversation_id)
            .bind(role_name)
            .fetch_one(tx.conn())
            .await
            .map_err(|e| RelationError::RoleNotFound(format!("{role_name} not found: {e}")))?;
        let role_id: i64 = row
            .try_get("role_id")
            .map_err(|e| RelationError::Store(format!("i64 role decode: {e}")))?;

        // 2) Ensure membership record exists.
        sqlx::query(
            r#"
INSERT INTO conversation_member (conversation_id, user_id)
VALUES (?, ?)
ON DUPLICATE KEY UPDATE last_read_off = last_read_off
"#,
        )
            .bind(conversation_id)
            .bind(user_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("ensure membership: {e}")))?;

        // 3) Assign role to member
        sqlx::query(
            r#"
INSERT INTO conversation_member_role (conversation_id, user_id, role_id)
VALUES (?, ?, ?)
ON DUPLICATE KEY UPDATE role_id = VALUES(role_id)
"#,
        )
            .bind(conversation_id)
            .bind(user_id)
            .bind(role_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("assign role: {e}")))?;

        Ok(())
    }

    async fn membership_exists_in_tx<'t>(
        &self,
        tx: &mut dyn StorageTx<'t>,
        conversation_id: ConversationId,
        user_id: UserId,
    ) -> Result<bool, RelationError> {
        let tx = downcast(tx);

        tracing::debug!(
            "membership check: uid: {}, cid: {}",
            user_id.0.to_string(),
            conversation_id.0.to_string()
        );

        let cnt: i64 = sqlx::query_scalar(
            r#"
SELECT COUNT(1)
FROM conversation_member
WHERE conversation_id = ? AND user_id = ?
"#,
        )
            .bind(conversation_id)
            .bind(user_id)
            .fetch_one(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("membership check: {}", e.to_string())))?;

        if cnt > 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}