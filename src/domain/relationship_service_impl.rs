use std::sync::Arc;
use uuid::Uuid;
use crate::domain::{ConversationId, FriendCursor, FriendSummary, GroupCursor, GroupId, GroupMemberRole, GroupSummary, IdempotencyKey, MemberCursor, MemberSummary, PageSize, RelationError, RelationshipService, TxManager, UserId};
use crate::infra::{ConversationRepo, ConversationRoleRepo, EventType, FriendshipIdemClaim, FriendshipNew, FriendshipRepo, GroupIdemClaim, GroupIdemRepo, GroupIdemStatus, GroupMemberNew, GroupNew, GroupRepo, OutboxEvent, OutboxRepo, S2CEvent, UserRepo};

pub struct RealRelationshipService {
    user_repo: Arc<dyn UserRepo>,
    friendship_repo: Arc<dyn FriendshipRepo>,
    group_repo: Arc<dyn GroupRepo>,
    group_idem_repo: Arc<dyn GroupIdemRepo>,
    conversation_repo: Arc<dyn ConversationRepo>,
    conversation_role_repo: Arc<dyn ConversationRoleRepo>,
    outbox_repo: Arc<dyn OutboxRepo>,
    tx_manager: Arc<dyn TxManager>,
}

impl RealRelationshipService {
    pub fn new(
        user_repo: Arc<dyn UserRepo>,
        friendship_repo: Arc<dyn FriendshipRepo>,
        group_repo: Arc<dyn GroupRepo>,
        group_idem_repo: Arc<dyn GroupIdemRepo>,
        conversation_repo: Arc<dyn ConversationRepo>,
        conversation_role_repo: Arc<dyn ConversationRoleRepo>,
        outbox_repo: Arc<dyn OutboxRepo>,
        tx_manager: Arc<dyn TxManager>,
    ) -> Self {
        Self {
            user_repo,
            friendship_repo,
            group_repo,
            group_idem_repo,
            conversation_repo,
            conversation_role_repo,
            outbox_repo,
            tx_manager,
        }
    }

    async fn create_group_internal(&self, owner: UserId, name: &str, description: Option<&str>, idempotency_key: IdempotencyKey, group_id: GroupId) -> Result<(GroupId, ConversationId), RelationError> {
        // Winner: all writes in ONE tx
        let mut tx = self
            .tx_manager
            .begin()
            .await
            .map_err(|e| RelationError::Store(e.to_string()))?;
        let conversation_id = ConversationId(Uuid::new_v4());

        // order matters (to reduce deadlock surface): conversation -> group -> roles
        self.conversation_repo
            .create_group_conversation_in_tx(&mut *tx, conversation_id)
            .await?;
        self.group_repo
            .insert_chat_group_in_tx(
                &mut *tx,
                group_id,
                owner,
                name,
                description,
                conversation_id,
            )
            .await?;
        self.conversation_role_repo
            .ensure_defaults_in_tx(&mut *tx, conversation_id)
            .await?;
        self.conversation_role_repo
            .assign_role_by_name_in_tx(&mut *tx, conversation_id, owner, "owner")
            .await?;

        tx.commit()
            .await
            .map_err(|e| RelationError::Store(e.to_string()))?;

        Ok((group_id, conversation_id))
    }
}

#[async_trait::async_trait]
impl RelationshipService for RealRelationshipService {
    async fn add_friend(
        &self,
        me: UserId,
        other: UserId,
        _idempotency_key: IdempotencyKey,
    ) -> std::result::Result<ConversationId, RelationError> {
        // claim friendship
        match self.friendship_repo.claim(me, other, me).await? {
            FriendshipIdemClaim::Won => {
                // Winner: all writes in ONE tx
                let mut tx = self
                    .tx_manager
                    .begin()
                    .await
                    .map_err(|e| RelationError::Store(e.to_string()))?;
                let proposed_conv_id = ConversationId(Uuid::new_v4());

                // order matters: conversation -> friendship
                self.conversation_repo
                    .create_direct_conversation_in_tx(&mut *tx, me, other, proposed_conv_id)
                    .await?;
                self.friendship_repo
                    .insert_friendship_in_tx(&mut *tx, me, other, proposed_conv_id)
                    .await?;

                let username = self
                    .user_repo
                    .get_username_in_tx(&mut *tx, me)
                    .await
                    .map_err(|e| {
                        tracing::warn!("query username: {e}");
                        RelationError::UserNotFound
                    })?;

                let event = OutboxEvent::new(
                    EventType::FriendshipNew,
                    Some(proposed_conv_id.0),
                    vec![other],
                    &S2CEvent::FriendshipNew(FriendshipNew {
                        conversation_id: proposed_conv_id,
                        other: me,
                        username,
                    }),
                )
                    .map_err(|e| RelationError::Store(e.to_string()))?;
                self.outbox_repo
                    .enqueue_in_tx(&mut *tx, &event)
                    .await
                    .map_err(|e| RelationError::Store(e.to_string()))?;

                tx.commit()
                    .await
                    .map_err(|e| RelationError::Store(e.to_string()))?;

                Ok(proposed_conv_id)
            }
            FriendshipIdemClaim::Existing => {
                // follower: read source of truth
                match self
                    .friendship_repo
                    .get_conversation_id_by_friendship(me, other)
                    .await
                {
                    Ok(conv_id) => Ok(conv_id),
                    Err(_) => Err(RelationError::Store(
                        "inconsistent friendship state".to_string(),
                    )),
                }
            }
        }
    }

    async fn list_friends(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<FriendCursor>,
    ) -> std::result::Result<Vec<FriendSummary>, RelationError> {
        Ok(self
            .friendship_repo
            .list_friends_with_conversations(user_id, page_size, after)
            .await?)
    }

    async fn create_group(
        &self,
        owner: UserId,
        name: &str,
        description: Option<&str>,
        idempotency_key: IdempotencyKey,
    ) -> Result<(GroupId, ConversationId), RelationError> {
        // claim key
        let proposed_gid = GroupId(uuid::Uuid::new_v4());
        match self
            .group_idem_repo
            .claim(owner, idempotency_key, proposed_gid)
            .await?
        {
            GroupIdemClaim::Won { group_id } => {
                let result = self.create_group_internal(owner, name, description, idempotency_key, group_id).await;
                match result {
                    // best-effort mark
                    Ok(pair) => {
                        let _ = self
                            .group_idem_repo
                            .mark_succeeded(owner, idempotency_key, pair.0, pair.1)
                            .await;
                        Ok(pair)
                    }
                    Err(e) => {
                        let _ = self
                            .group_idem_repo
                            .mark_failed(owner, idempotency_key, group_id, &e.to_string().chars().take(64).collect::<String>())
                            .await;
                        Err(RelationError::Store("group creation failed".to_owned()))
                    }
                }
            }
            GroupIdemClaim::Existing {
                group_id,
                status: GroupIdemStatus::Succeeded,
                conversation_id: Some(conv_id),
            } => {
                // follower: return cached value
                Ok((group_id, conv_id))
            }
            GroupIdemClaim::Existing {
                group_id,
                status: GroupIdemStatus::Succeeded,
                conversation_id: None,
            } | GroupIdemClaim::Existing {
                group_id,
                status: GroupIdemStatus::Pending,
                ..
            } => {
                // follower: read source of truth if cache is not ready
                if let Some(conv_id) = self
                    .group_repo
                    .get_conversation_id_by_group(group_id)
                    .await?
                {
                    let _ = self
                        .group_idem_repo
                        .mark_succeeded(owner, idempotency_key, group_id, conv_id)
                        .await;
                    return Ok((group_id, conv_id));
                }
                Err(RelationError::Store(
                    "inconsistent idempotency state".to_string(),
                ))
            }
            GroupIdemClaim::Existing {
                group_id,
                status: GroupIdemStatus::Failed,
                ..
            } => Err(RelationError::Store(format!(
                "previous attempt failed for group {}",
                group_id
            ))),
        }
    }

    async fn invite_to_group(
        &self,
        group: GroupId,
        host: UserId,
        guest: UserId,
    ) -> std::result::Result<(), RelationError> {
        let conversation_id = self
            .group_repo
            .get_conversation_id_by_group(group)
            .await?
            .ok_or(RelationError::Store(format!(
                "inconsistent group conversation state for {group}"
            )))?;
        let role = self
            .conversation_role_repo
            .get_role_by_conversation_id(host, conversation_id)
            .await?;
        if !matches!(role, GroupMemberRole::Owner) {
            return Err(RelationError::NotOwner);
        }

        let mut tx = self
            .tx_manager
            .begin()
            .await
            .map_err(|e| RelationError::Store(e.to_string()))?;

        self.conversation_role_repo
            .assign_role_by_name_in_tx(&mut *tx, conversation_id, guest, "member")
            .await?;

        // push to guest
        let group_summary = self.group_repo.get_group_summary_in_tx(&mut *tx, group).await?;
        let event = OutboxEvent::new(
            EventType::GroupNew,
            Some(conversation_id.0),
            vec![guest],
            &S2CEvent::GroupNew(GroupNew {
                conversation_id,
                group_id: group,
                group_name: group_summary.name,
            })
        ).map_err(|e| RelationError::Store(format!("compose group.new event: {e}")))?;
        self.outbox_repo.enqueue_in_tx(&mut *tx, &event).await
            .map_err(|e| RelationError::Store(format!("enqueue group.new event to outbox: {e}")))?;

        // push to other members
        let username = self
            .user_repo
            .get_username_in_tx(&mut *tx, guest)
            .await
            .map_err(|e| RelationError::Store(e.to_string()))?;
        let members = self
            .group_repo
            .list_group_members_in_tx(&mut *tx, group, PageSize(100), None)
            .await?;
        let mut receivers = Vec::with_capacity(members.len() - 1);
        for member in members {
            if member.user_id != host {
                receivers.push(member.user_id);
            }
        }
        if !receivers.is_empty() {
            let event = OutboxEvent::new(
                EventType::GroupMemberNew,
                Some(conversation_id.0),
                receivers,
                &S2CEvent::GroupMemberNew(GroupMemberNew {
                    conversation_id,
                    group_id: group,
                    member_id: guest,
                    username,
                }),
            )
                .map_err(|e| RelationError::Store(format!("compose group.member.new event: {e}")))?;
            // new feature hint: load all, trunking logic
            self.outbox_repo
                .enqueue_in_tx(&mut *tx, &event)
                .await
                .map_err(|e| RelationError::Store(format!("enqueue group.member.new event to outbox: {e}")))?;
        }

        tx.commit()
            .await
            .map_err(|e| RelationError::Store(e.to_string()))?;

        Ok(())
    }

    async fn list_groups(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<GroupCursor>,
    ) -> std::result::Result<Vec<GroupSummary>, RelationError> {
        self.group_repo.list_groups(user_id, page_size, after).await
    }

    async fn list_group_members(
        &self,
        _user_id: UserId,
        group: GroupId,
        page_size: PageSize,
        after: Option<MemberCursor>,
    ) -> std::result::Result<Vec<MemberSummary>, RelationError> {
        let mut tx = self
            .tx_manager
            .begin()
            .await
            .map_err(|e| RelationError::Store(e.to_string()))?;

        let summary = self
            .group_repo
            .list_group_members_in_tx(&mut *tx, group, page_size, after)
            .await?;

        tx.commit()
            .await
            .map_err(|e| RelationError::Store(e.to_string()))?;

        Ok(summary)
    }
}