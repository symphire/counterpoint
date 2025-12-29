use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row};
use crate::domain::{ConversationId, GroupCursor, GroupId, GroupMemberRole, GroupSummary, MemberCursor, MemberSummary, PageSize, RelationError, StorageTx, UserId};
use crate::infra::{downcast, GroupRepo, GroupShortSummary};

pub struct MySqlGroupRepo {
    pool: MySqlPool,
}

impl MySqlGroupRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GroupRepo for MySqlGroupRepo {
    async fn get_group_summary_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        group_id: GroupId,
    ) -> Result<GroupShortSummary, RelationError> {
        #[derive(sqlx::FromRow)]
        struct GroupRow {
            group_id: GroupId,
            group_name: String,
            conversation_id: ConversationId,
        }

        let tx = downcast(tx);

        let row = sqlx::query_as!(
            GroupRow,
            r#"
SELECT group_id AS "group_id: GroupId", group_name AS "group_name: String", conversation_id AS "conversation_id: ConversationId"
FROM chat_group
WHERE group_id = ?
"#,
            group_id
        ).fetch_optional(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("group details of {group_id}: {e}")))?;

        row.map(|r| GroupShortSummary {
            group_id: r.group_id,
            name: r.group_name,
            conversation_id: r.conversation_id,
        })
            .ok_or(RelationError::GroupNotFound)
    }

    async fn insert_chat_group_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        group_id: GroupId,
        owner: UserId,
        name: &str,
        description: Option<&str>,
        conversation_id: ConversationId,
    ) -> Result<(), RelationError> {
        let tx = downcast(tx);

        sqlx::query(
            r#"
INSERT INTO chat_group (group_id, owner_id, group_name, description, conversation_id)
VALUES (?, ?, ?, ?, ?)
"#,
        )
            .bind(group_id)
            .bind(owner)
            .bind(name)
            .bind(description)
            .bind(conversation_id)
            .execute(tx.conn())
            .await
            .map_err(|e| RelationError::Store(format!("insert chat group: {e}")))?;

        Ok(())
    }

    async fn get_conversation_id_by_group(
        &self,
        group_id: GroupId,
    ) -> Result<Option<ConversationId>, RelationError> {
        let row = sqlx::query("SELECT conversation_id FROM chat_group WHERE group_id = ?")
            .bind(group_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RelationError::Store(format!("select chat_group: {e}")))?;

        let cid = row
            .map(|r| {
                r.try_get::<ConversationId, _>("conversation_id")
                    .map_err(|e| RelationError::Store(format!("uuid conv decode: {e}")))
            })
            .transpose()?;

        Ok(cid)
    }

    async fn list_groups(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<GroupCursor>,
    ) -> Result<Vec<GroupSummary>, RelationError> {
        #[derive(sqlx::FromRow)]
        struct GroupRow {
            group_id: GroupId,
            group_name: String,
            conversation_id: ConversationId,
            created_at: DateTime<Utc>,
            is_owner: i32,     // (cg.owner_id = ?) -> 0 or 1
            member_count: i64, // COUNT(*) is BIGINT
        }

        let ps = page_size.0 as i64;

        let rows: Vec<GroupRow> = if let Some(cursor) = after {
            // With cursor
            sqlx::query_as::<_, GroupRow>(
                r#"
SELECT
    cg.group_id,
    cg.group_name,
    cg.conversation_id,
    cg.created_at,
    (cg.owner_id = ?) AS is_owner,
    mc.member_count
FROM chat_group cg
JOIN conversation_member cm
  ON cm.conversation_id = cg.conversation_id
 AND cm.user_id = ?
JOIN (
    SELECT conversation_id, COUNT(*) AS member_count
    FROM conversation_member
    GROUP BY conversation_id
) mc ON mc.conversation_id = cg.conversation_id
WHERE
    (cg.created_at < ?)
    OR (cg.created_at = ? AND cg.group_id < ?)
ORDER BY cg.created_at DESC, cg.group_id DESC
LIMIT ?
"#,
            )
                .bind(user_id) // for (cg.owner_id = ?)
                .bind(user_id) // for cm.user_id = ?
                .bind(cursor.created_at) // cg.created_at < ?
                .bind(cursor.created_at) // cg.created_at = ?
                .bind(cursor.group_id) // cg.group_id < ?
                .bind(ps)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RelationError::Store(format!("list_groups(after) query: {e}")))?
        } else {
            sqlx::query_as::<_, GroupRow>(
                r#"
SELECT
  cg.group_id,
  cg.group_name,
  cg.conversation_id,
  cg.created_at,
  (cg.owner_id = ?) AS is_owner,
  mc.member_count
FROM chat_group cg
JOIN conversation_member cm
  ON cm.conversation_id = cg.conversation_id
 AND cm.user_id = ?
JOIN (
  SELECT conversation_id, COUNT(*) AS member_count
  FROM conversation_member
  GROUP BY conversation_id
) mc ON mc.conversation_id = cg.conversation_id
ORDER BY cg.created_at DESC, cg.group_id DESC
LIMIT ?
                "#,
            )
                .bind(user_id) // for (cg.owner_id = ?)
                .bind(user_id) // for cm.user_id = ?
                .bind(ps)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RelationError::Store(format!("list_groups(first) query: {e}")))?
        };

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let my_role = if r.is_owner != 0 {
                GroupMemberRole::Owner
            } else {
                GroupMemberRole::Member
            };

            let member_count = u32::try_from(r.member_count).map_err(|_| {
                RelationError::Store(format!("member_count overflow: {}", r.member_count))
            })?;

            out.push(GroupSummary {
                group_id: r.group_id,
                name: r.group_name,
                my_role,
                conversation_id: r.conversation_id,
                member_count,
                created_at: r.created_at,
            })
        }

        Ok(out)
    }

    async fn list_group_members_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        group: GroupId,
        page_size: PageSize,
        after: Option<MemberCursor>,
    ) -> Result<Vec<MemberSummary>, RelationError> {
        #[derive(sqlx::FromRow)]
        struct MemberRow {
            user_id: UserId,
            username: String,
            joined_at: DateTime<Utc>,
        }

        // 1) Resolve conversation_id from group_id
        let conv_id: Option<ConversationId> =
            sqlx::query_scalar(r#"SELECT conversation_id FROM chat_group WHERE group_id = ?"#)
                .bind(group)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| RelationError::Store(format!("resolve conv_id: {e}")))?;

        let conv_id = match conv_id {
            Some(id) => id,
            None => {
                return Err(RelationError::Store(format!(
                    "inconsistent group conversation: {group}"
                )));
            }
        };

        // 2) Query
        let ps = page_size.0 as i64;

        let rows: Vec<MemberRow> = if let Some(cur) = after {
            sqlx::query_as::<_, MemberRow>(
                r#"
SELECT cm.user_id, u.username, cm.joined_at
FROM conversation_member cm
JOIN user u ON u.user_id = cm.user_id
WHERE cm.conversation_id = ?
  AND ( cm.joined_at < ? OR (cm.joined_at = ? AND cm.user_id < ?) )
ORDER BY cm.joined_at DESC, cm.user_id DESC
LIMIT ?
                "#,
            )
                .bind(conv_id)
                .bind(cur.joined_at)
                .bind(cur.joined_at)
                .bind(cur.user)
                .bind(ps)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RelationError::Store(format!("list_group_members(after): {e}")))?
        } else {
            sqlx::query_as::<_, MemberRow>(
                r#"
SELECT cm.user_id, u.username, cm.joined_at
FROM conversation_member cm
JOIN user u ON u.user_id = cm.user_id
WHERE cm.conversation_id = ?
ORDER BY cm.joined_at DESC, cm.user_id DESC
LIMIT ?
                "#,
            )
                .bind(conv_id)
                .bind(ps)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RelationError::Store(format!("list_group_members(first): {e}")))?
        };

        // 3) Map to DTO
        let out = rows
            .into_iter()
            .map(|r| MemberSummary {
                user_id: r.user_id,
                username: r.username,
                joined_at: r.joined_at,
            })
            .collect();

        Ok(out)
    }
}