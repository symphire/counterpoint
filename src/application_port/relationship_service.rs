use crate::domain_model::*;

#[derive(Debug, thiserror::Error)]
pub enum RelationError {
    #[error("user not found")]
    UserNotFound,
    #[error("friend request already exists")]
    FriendRequestExists,
    #[error("friendship already established")]
    AlreadyFriends,
    #[error("group not found")]
    GroupNotFound,
    #[error("already a member")]
    AlreadyMember,
    #[error("not a member")]
    NotMember,
    #[error("not an owner")]
    NotOwner,
    #[error("role not found: {0}")]
    RoleNotFound(String),
    #[error("store error: {0}")]
    Store(String),
}

#[async_trait::async_trait]
pub trait RelationshipService: Send + Sync {
    async fn add_friend(
        &self,
        me: UserId,
        other: UserId,
        _idempotency_key: IdempotencyKey,
    ) -> Result<ConversationId, RelationError>;
    async fn list_friends(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<FriendCursor>,
    ) -> Result<Vec<FriendSummary>, RelationError>;
    async fn create_group(
        &self,
        owner: UserId,
        name: &str,
        description: Option<&str>,
        idempotency_key: IdempotencyKey,
    ) -> Result<(GroupId, ConversationId), RelationError>;
    async fn invite_to_group(
        &self,
        group: GroupId,
        host: UserId,
        guest: UserId,
    ) -> Result<(), RelationError>;
    async fn list_groups(
        &self,
        user_id: UserId,
        page_size: PageSize,
        after: Option<GroupCursor>,
    ) -> Result<Vec<GroupSummary>, RelationError>;
    async fn list_group_members(
        &self,
        user_id: UserId,
        group: GroupId,
        page_size: PageSize,
        after: Option<MemberCursor>,
    ) -> Result<Vec<MemberSummary>, RelationError>;
}
