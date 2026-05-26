use crate::domain_model::*;

#[derive(Debug, thiserror::Error)]
pub enum RelationError {
    #[error("user not found")]
    UserNotFound,
    #[error("friend request already exists")]
    FriendRequestExists,
    #[error("friend request not found")]
    FriendRequestNotFound,
    #[error("invitation not found")]
    InvitationNotFound,
    #[error("join request not found")]
    JoinRequestNotFound,
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
    async fn send_friend_request(
        &self,
        me: UserId,
        other: UserId,
    ) -> Result<(), RelationError>;
    async fn accept_friend_request(
        &self,
        me: UserId,
        requester: UserId,
    ) -> Result<ConversationId, RelationError>;
    async fn reject_friend_request(
        &self,
        me: UserId,
        requester: UserId,
    ) -> Result<(), RelationError>;
    async fn list_incoming_friend_requests(
        &self,
        me: UserId,
        page_size: PageSize,
        after: Option<FriendRequestCursor>,
    ) -> Result<Vec<FriendRequestSummary>, RelationError>;
    async fn remove_friend(&self, me: UserId, other: UserId) -> Result<(), RelationError>;
    async fn invite_to_group_v2(
        &self,
        group: GroupId,
        host: UserId,
        guest: UserId,
    ) -> Result<GroupInvitationId, RelationError>;
    async fn accept_group_invitation(
        &self,
        me: UserId,
        invitation_id: GroupInvitationId,
    ) -> Result<(), RelationError>;
    async fn reject_group_invitation(
        &self,
        me: UserId,
        invitation_id: GroupInvitationId,
    ) -> Result<(), RelationError>;
    async fn list_group_invitations(
        &self,
        me: UserId,
        page_size: PageSize,
        after: Option<GroupInvitationCursor>,
    ) -> Result<Vec<GroupInvitationSummary>, RelationError>;
    async fn request_to_join_group(
        &self,
        me: UserId,
        group_id: GroupId,
    ) -> Result<GroupJoinRequestId, RelationError>;
    async fn accept_join_request(
        &self,
        me: UserId,
        request_id: GroupJoinRequestId,
    ) -> Result<(), RelationError>;
    async fn reject_join_request(
        &self,
        me: UserId,
        request_id: GroupJoinRequestId,
    ) -> Result<(), RelationError>;
    async fn list_join_requests(
        &self,
        me: UserId,
        group_id: GroupId,
        page_size: PageSize,
        after: Option<GroupJoinRequestCursor>,
    ) -> Result<Vec<GroupJoinRequestSummary>, RelationError>;
    async fn leave_group(&self, me: UserId, group_id: GroupId) -> Result<(), RelationError>;
    async fn remove_group_member(
        &self,
        me: UserId,
        group_id: GroupId,
        target: UserId,
    ) -> Result<(), RelationError>;
    async fn transfer_group_ownership(
        &self,
        me: UserId,
        group_id: GroupId,
        new_owner: UserId,
    ) -> Result<(), RelationError>;
    async fn update_group_info(
        &self,
        me: UserId,
        group_id: GroupId,
        name: &str,
        description: Option<&str>,
    ) -> Result<(), RelationError>;
}
