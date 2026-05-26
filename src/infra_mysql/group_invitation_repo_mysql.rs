use super::util::downcast;
use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::*;
use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row};

pub struct MySqlGroupInvitationRepo {
    pool: MySqlPool,
}

impl MySqlGroupInvitationRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GroupInvitationRepo for MySqlGroupInvitationRepo {
    async fn insert(
        &self,
        invitation_id: GroupInvitationId,
        group_id: GroupId,
        inviter_id: UserId,
        invitee_id: UserId,
    ) -> Result<(), RelationError> {
        sqlx::query(
            r#"
INSERT INTO group_invitation (invitation_id, group_id, inviter_id, invitee_id, status, created_at)
VALUES (?, ?, ?, ?, 'pending', NOW(6))
ON DUPLICATE KEY UPDATE
    invitation_id = VALUES(invitation_id),
    inviter_id    = VALUES(inviter_id),
    status        = 'pending',
    created_at    = NOW(6)
"#,
        )
        .bind(invitation_id)
        .bind(group_id)
        .bind(inviter_id)
        .bind(invitee_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("insert group invitation: {e}")))?;

        Ok(())
    }

    async fn accept_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        invitation_id: GroupInvitationId,
        invitee_id: UserId,
    ) -> Result<GroupId, RelationError> {
        let tx = downcast(tx);

        let result = sqlx::query(
            "UPDATE group_invitation SET status = 'accepted' WHERE invitation_id = ? AND invitee_id = ? AND status = 'pending'",
        )
        .bind(invitation_id)
        .bind(invitee_id)
        .execute(tx.conn())
        .await
        .map_err(|e| RelationError::Store(format!("accept group invitation: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(RelationError::InvitationNotFound);
        }

        let row = sqlx::query(
            "SELECT group_id FROM group_invitation WHERE invitation_id = ?",
        )
        .bind(invitation_id)
        .fetch_one(tx.conn())
        .await
        .map_err(|e| RelationError::Store(format!("fetch group_id from invitation: {e}")))?;

        Ok(row.get::<GroupId, _>("group_id"))
    }

    async fn reject(
        &self,
        invitation_id: GroupInvitationId,
        invitee_id: UserId,
    ) -> Result<(), RelationError> {
        let result = sqlx::query(
            "UPDATE group_invitation SET status = 'rejected' WHERE invitation_id = ? AND invitee_id = ? AND status = 'pending'",
        )
        .bind(invitation_id)
        .bind(invitee_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("reject group invitation: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(RelationError::InvitationNotFound);
        }

        Ok(())
    }

    async fn list_for_invitee(
        &self,
        invitee_id: UserId,
        page_size: PageSize,
        after: Option<GroupInvitationCursor>,
    ) -> Result<Vec<GroupInvitationSummary>, RelationError> {
        let limit = page_size.0 as i64;

        let rows = if let Some(cur) = after {
            sqlx::query(
                r#"
SELECT gi.invitation_id, gi.group_id, cg.group_name, gi.inviter_id, u.username AS inviter_username, gi.created_at
FROM group_invitation gi
JOIN chat_group cg ON cg.group_id = gi.group_id
JOIN user u ON u.user_id = gi.inviter_id
WHERE gi.invitee_id = ? AND gi.status = 'pending'
  AND (gi.created_at > ? OR (gi.created_at = ? AND gi.invitation_id > ?))
ORDER BY gi.created_at ASC, gi.invitation_id ASC
LIMIT ?
"#,
            )
            .bind(invitee_id)
            .bind(cur.created_at)
            .bind(cur.created_at)
            .bind(cur.invitation_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelationError::Store(format!("list group invitations: {e}")))?
        } else {
            sqlx::query(
                r#"
SELECT gi.invitation_id, gi.group_id, cg.group_name, gi.inviter_id, u.username AS inviter_username, gi.created_at
FROM group_invitation gi
JOIN chat_group cg ON cg.group_id = gi.group_id
JOIN user u ON u.user_id = gi.inviter_id
WHERE gi.invitee_id = ? AND gi.status = 'pending'
ORDER BY gi.created_at ASC, gi.invitation_id ASC
LIMIT ?
"#,
            )
            .bind(invitee_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelationError::Store(format!("list group invitations: {e}")))?
        };

        let summaries = rows
            .iter()
            .map(|r| GroupInvitationSummary {
                invitation_id: r.get::<GroupInvitationId, _>("invitation_id"),
                group_id: r.get::<GroupId, _>("group_id"),
                group_name: r.get::<String, _>("group_name"),
                inviter_id: r.get::<UserId, _>("inviter_id"),
                inviter_username: r.get::<String, _>("inviter_username"),
                created_at: r.get::<DateTime<Utc>, _>("created_at"),
            })
            .collect();

        Ok(summaries)
    }
}

pub struct MySqlGroupJoinRequestRepo {
    pool: MySqlPool,
}

impl MySqlGroupJoinRequestRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GroupJoinRequestRepo for MySqlGroupJoinRequestRepo {
    async fn insert(
        &self,
        request_id: GroupJoinRequestId,
        group_id: GroupId,
        requester_id: UserId,
    ) -> Result<(), RelationError> {
        sqlx::query(
            r#"
INSERT INTO group_join_request (request_id, group_id, requester_id, status, created_at)
VALUES (?, ?, ?, 'pending', NOW(6))
ON DUPLICATE KEY UPDATE
    request_id   = VALUES(request_id),
    status       = 'pending',
    created_at   = NOW(6)
"#,
        )
        .bind(request_id)
        .bind(group_id)
        .bind(requester_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("insert group join request: {e}")))?;

        Ok(())
    }

    async fn find_pending_group(
        &self,
        request_id: GroupJoinRequestId,
    ) -> Result<GroupId, RelationError> {
        let row = sqlx::query(
            "SELECT group_id FROM group_join_request WHERE request_id = ? AND status = 'pending'",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("find pending join request: {e}")))?
        .ok_or(RelationError::JoinRequestNotFound)?;

        Ok(row.get::<GroupId, _>("group_id"))
    }

    async fn accept_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        request_id: GroupJoinRequestId,
        group_id: GroupId,
    ) -> Result<UserId, RelationError> {
        let tx = downcast(tx);

        let result = sqlx::query(
            "UPDATE group_join_request SET status = 'accepted' WHERE request_id = ? AND group_id = ? AND status = 'pending'",
        )
        .bind(request_id)
        .bind(group_id)
        .execute(tx.conn())
        .await
        .map_err(|e| RelationError::Store(format!("accept group join request: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(RelationError::JoinRequestNotFound);
        }

        let row = sqlx::query(
            "SELECT requester_id FROM group_join_request WHERE request_id = ?",
        )
        .bind(request_id)
        .fetch_one(tx.conn())
        .await
        .map_err(|e| RelationError::Store(format!("fetch requester from join request: {e}")))?;

        Ok(row.get::<UserId, _>("requester_id"))
    }

    async fn reject(
        &self,
        request_id: GroupJoinRequestId,
        group_id: GroupId,
    ) -> Result<(), RelationError> {
        let result = sqlx::query(
            "UPDATE group_join_request SET status = 'rejected' WHERE request_id = ? AND group_id = ? AND status = 'pending'",
        )
        .bind(request_id)
        .bind(group_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RelationError::Store(format!("reject group join request: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(RelationError::JoinRequestNotFound);
        }

        Ok(())
    }

    async fn list_for_group(
        &self,
        group_id: GroupId,
        page_size: PageSize,
        after: Option<GroupJoinRequestCursor>,
    ) -> Result<Vec<GroupJoinRequestSummary>, RelationError> {
        let limit = page_size.0 as i64;

        let rows = if let Some(cur) = after {
            sqlx::query(
                r#"
SELECT gjr.request_id, gjr.group_id, gjr.requester_id, u.username AS requester_username, gjr.created_at
FROM group_join_request gjr
JOIN user u ON u.user_id = gjr.requester_id
WHERE gjr.group_id = ? AND gjr.status = 'pending'
  AND (gjr.created_at > ? OR (gjr.created_at = ? AND gjr.request_id > ?))
ORDER BY gjr.created_at ASC, gjr.request_id ASC
LIMIT ?
"#,
            )
            .bind(group_id)
            .bind(cur.created_at)
            .bind(cur.created_at)
            .bind(cur.request_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelationError::Store(format!("list group join requests: {e}")))?
        } else {
            sqlx::query(
                r#"
SELECT gjr.request_id, gjr.group_id, gjr.requester_id, u.username AS requester_username, gjr.created_at
FROM group_join_request gjr
JOIN user u ON u.user_id = gjr.requester_id
WHERE gjr.group_id = ? AND gjr.status = 'pending'
ORDER BY gjr.created_at ASC, gjr.request_id ASC
LIMIT ?
"#,
            )
            .bind(group_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelationError::Store(format!("list group join requests: {e}")))?
        };

        let summaries = rows
            .iter()
            .map(|r| GroupJoinRequestSummary {
                request_id: r.get::<GroupJoinRequestId, _>("request_id"),
                group_id: r.get::<GroupId, _>("group_id"),
                requester_id: r.get::<UserId, _>("requester_id"),
                requester_username: r.get::<String, _>("requester_username"),
                created_at: r.get::<DateTime<Utc>, _>("created_at"),
            })
            .collect();

        Ok(summaries)
    }
}
