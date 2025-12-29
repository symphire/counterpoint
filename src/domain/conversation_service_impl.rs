use std::sync::Arc;
use crate::domain::{ChatError, ConversationId, ConversationService, MessageId, MessageRecord, OffsetCursor, PageSize, RecentConversation, TimeCursor, TxManager, UserId};
use crate::infra::{ChatMessageNew, ConversationRepo, ConversationRoleRepo, EventType, MessageRepo, OutboxEvent, OutboxRepo, S2CEvent, UserRepo};

pub struct FakeConversationService;

#[async_trait::async_trait]
impl ConversationService for FakeConversationService {
    async fn send_message(&self, conversation_id: ConversationId, sender: UserId, content: &str, message_id: MessageId) -> Result<MessageRecord, ChatError> {
        todo!()
    }

    async fn get_history(&self, user_id: UserId, conversation_id: ConversationId, page_size: PageSize, before: Option<OffsetCursor>) -> Result<Vec<MessageRecord>, ChatError> {
        todo!()
    }

    async fn recent_conversations(&self, user_id: UserId, page_size: PageSize, after: Option<TimeCursor>) -> Result<Vec<RecentConversation>, ChatError> {
        todo!()
    }
}

pub struct RealConversationService {
    user_repo: Arc<dyn UserRepo>,
    message_repo: Arc<dyn MessageRepo>,
    conversation_repo: Arc<dyn ConversationRepo>,
    conversation_role_repo: Arc<dyn ConversationRoleRepo>,
    outbox_repo: Arc<dyn OutboxRepo>,
    tx_manager: Arc<dyn TxManager>,
}

impl RealConversationService {
    pub fn new(
        user_repo: Arc<dyn UserRepo>,
        message_repo: Arc<dyn MessageRepo>,
        conversation_repo: Arc<dyn ConversationRepo>,
        conversation_role_repo: Arc<dyn ConversationRoleRepo>,
        outbox_repo: Arc<dyn OutboxRepo>,
        tx_manager: Arc<dyn TxManager>,
    ) -> Self {
        Self {
            user_repo,
            message_repo,
            conversation_repo,
            conversation_role_repo,
            outbox_repo,
            tx_manager,
        }
    }
}

#[async_trait::async_trait]
impl ConversationService for RealConversationService {
    async fn send_message(
        &self,
        conversation_id: ConversationId,
        sender: UserId,
        content: &str,
        message_id: MessageId,
    ) -> Result<MessageRecord, ChatError> {
        let mut tx = self
            .tx_manager
            .begin()
            .await
            .map_err(|e| ChatError::Store(e.to_string()))?;

        let is_member = self
            .conversation_role_repo
            .membership_exists_in_tx(&mut *tx, conversation_id, sender)
            .await
            .map_err(|e| ChatError::Store(e.to_string()))?;
        if !is_member {
            tracing::trace!("membership check failed when sending message");
            return Err(ChatError::NotMember);
        }

        let record = self
            .message_repo
            .insert_in_tx(&mut *tx, conversation_id, sender, content, message_id)
            .await?;

        let mut members = self.conversation_repo.get_conversation_member_in_tx(&mut *tx, conversation_id).await
            .map_err(|e| ChatError::Store(format!("query chat members: {e}")))?;
        let mut receivers = Vec::with_capacity(members.len());
        for member in members {
            if member != sender {
                receivers.push(member);
            }
        }

        let username = self.user_repo.get_username_in_tx(&mut *tx, record.sender).await
            .map_err(|e| ChatError::Store(format!("query sender username: {e}")))?;

        let event = OutboxEvent::new(
            EventType::ChatMessageNew,
            Some(conversation_id.0),
            receivers,
            &S2CEvent::ChatMessageNew(ChatMessageNew {
                conversation_id: record.conversation_id,
                message_id: record.message_id,
                message_offset: record.message_offset,
                content: record.content.clone(),
                sender: record.sender,
                username,
                created_at: record.created_at,
            })
        ).map_err(|e| ChatError::Store(format!("compose chat.message.new event: {e}")))?;
        self.outbox_repo.enqueue_in_tx(&mut *tx, &event).await.map_err(|e| ChatError::Store(format!("enqueue chat.message.new event: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| ChatError::Store(e.to_string()))?;

        Ok(record)
    }

    async fn get_history(
        &self,
        user_id: UserId,
        conversation_id: ConversationId,
        page_size: PageSize,
        before: Option<OffsetCursor>,
    ) -> Result<Vec<MessageRecord>, ChatError> {
        let mut tx = self
            .tx_manager
            .begin()
            .await
            .map_err(|e| ChatError::Store(e.to_string()))?;

        let ok = self
            .conversation_role_repo
            .membership_exists_in_tx(&mut *tx, conversation_id, user_id)
            .await
            .map_err(|e| ChatError::Store(e.to_string()))?;
        if !ok {
            return Err(ChatError::NotMember);
        }

        let page = self
            .message_repo
            .list_before_in_tx(&mut *tx, conversation_id, page_size, before)
            .await?;

        tx.commit()
            .await
            .map_err(|e| ChatError::Store(e.to_string()))?;

        Ok(page)
    }

    async fn recent_conversations(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<TimeCursor>,
    ) -> std::result::Result<Vec<RecentConversation>, ChatError> {
        let mut tx = self
            .tx_manager
            .begin()
            .await
            .map_err(|e| ChatError::Store(e.to_string()))?;

        let ids = self
            .conversation_repo
            .list_for_user_recent_in_tx(&mut *tx, user_id, page_size, after)
            .await?;
        tracing::trace!("recent conversation ids: {:?}", ids);

        let conversations = if ids.is_empty() {
            vec![]
        } else {
            self.conversation_repo
                .hydrate_conversation_in_tx(&mut *tx, user_id, ids)
                .await?
        };

        tx.commit()
            .await
            .map_err(|e| ChatError::Store(e.to_string()))?;

        Ok(conversations)
    }
}