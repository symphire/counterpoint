use crate::application_port::*;
use crate::domain_model::*;
use crate::domain_port::repo_tx::StorageTx;

#[async_trait::async_trait]
pub trait GroupInvitationRepo: Send + Sync {
    async fn insert(
        &self,
        invitation_id: GroupInvitationId,
        group_id: GroupId,
        inviter_id: UserId,
        invitee_id: UserId,
    ) -> Result<(), RelationError>;

    /// Validates that invitation is pending and belongs to invitee_id.
    /// Updates status to 'accepted' within the transaction and returns group_id.
    async fn accept_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        invitation_id: GroupInvitationId,
        invitee_id: UserId,
    ) -> Result<GroupId, RelationError>;

    async fn reject(
        &self,
        invitation_id: GroupInvitationId,
        invitee_id: UserId,
    ) -> Result<(), RelationError>;

    async fn list_for_invitee(
        &self,
        invitee_id: UserId,
        page_size: PageSize,
        after: Option<GroupInvitationCursor>,
    ) -> Result<Vec<GroupInvitationSummary>, RelationError>;
}

#[async_trait::async_trait]
pub trait GroupJoinRequestRepo: Send + Sync {
    async fn insert(
        &self,
        request_id: GroupJoinRequestId,
        group_id: GroupId,
        requester_id: UserId,
    ) -> Result<(), RelationError>;

    /// Returns the GroupId of the pending request (for ownership validation before accepting).
    async fn find_pending_group(
        &self,
        request_id: GroupJoinRequestId,
    ) -> Result<GroupId, RelationError>;

    /// Updates status to 'accepted' within the transaction and returns requester_id.
    async fn accept_in_tx(
        &self,
        tx: &mut dyn StorageTx<'_>,
        request_id: GroupJoinRequestId,
        group_id: GroupId,
    ) -> Result<UserId, RelationError>;

    async fn reject(
        &self,
        request_id: GroupJoinRequestId,
        group_id: GroupId,
    ) -> Result<(), RelationError>;

    async fn list_for_group(
        &self,
        group_id: GroupId,
        page_size: PageSize,
        after: Option<GroupJoinRequestCursor>,
    ) -> Result<Vec<GroupJoinRequestSummary>, RelationError>;
}
