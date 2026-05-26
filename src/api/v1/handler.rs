use super::error::*;
use crate::application_port::*;
use crate::domain_model::*;
use crate::logger::*;
use crate::server::ConnectionAcceptor;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use warp::{self, reject};

/// TODO: This is currently a God File to help us move fast.
/// Refactor and tidy up when the feature set is more stable.

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(code: ApiErrorCode, message: impl Into<String>) -> Self {
        ApiResponse {
            success: false,
            data: None,
            error: Some(ApiError {
                code,
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Serialize)]
struct CaptchaResponse {
    id: uuid::Uuid,
    image_base64: String,
    expire_at: DateTime<Utc>,
}

pub async fn generate_captcha(
    captcha_service: Arc<dyn CaptchaService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let captcha = captcha_service
        .generate()
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;

    let response = CaptchaResponse {
        id: captcha.id.0,
        image_base64: captcha.image_base64,
        expire_at: captcha.expire_at,
    };
    Ok(warp::reply::json(&response))
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub captcha_id: uuid::Uuid,
    pub captcha_answer: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user_id: UserId,
    pub auth_tokens: AuthTokens,
}

pub async fn login(
    body: LoginRequest,
    auth_service: Arc<dyn AuthService>,
    captcha_service: Arc<dyn CaptchaService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let validation_input = ValidationInput {
        id: CaptchaId(body.captcha_id),
        answer: body.captcha_answer,
    };
    captcha_service
        .validate(validation_input)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;

    let login_input = LoginInput {
        username: body.username.clone(),
        password: body.password.clone(),
    };
    let login_result = auth_service
        .login(login_input)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;

    let login_response = LoginResponse {
        user_id: login_result.user_id,
        auth_tokens: login_result.tokens,
    };
    let api_response = ApiResponse::ok(login_response);

    Ok(warp::reply::json(&api_response))
}

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub username: String,
    pub password: String,
    pub captcha_id: uuid::Uuid,
    pub captcha_answer: String,
}

#[derive(Debug, Serialize)]
pub struct SignupResponse;

pub async fn signup(
    body: SignupRequest,
    auth_service: Arc<dyn AuthService>,
    captcha_service: Arc<dyn CaptchaService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let validation_input = ValidationInput {
        id: CaptchaId(body.captcha_id),
        answer: body.captcha_answer,
    };
    captcha_service
        .validate(validation_input)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;

    let signup_input = SignupInput {
        username: body.username,
        password: body.password,
    };
    let _user_id = auth_service
        .signup(signup_input)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;

    Ok(warp::reply::json(&ApiResponse::ok(SignupResponse)))
}

#[derive(Debug, Deserialize)]
pub struct FriendListQuery {
    pub page_size: PageSize,
    pub after: Option<String>,
}

pub async fn generate_friend_list(
    query: FriendListQuery,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let page_size = query.page_size;
    let after = query
        .after
        .map(|s| s.parse::<FriendCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;

    let summary = relationship_service
        .list_friends(user_id, page_size, after)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;

    let response = ApiResponse::ok(summary);
    Ok(warp::reply::json(&response))
}

#[derive(Debug, Deserialize)]
pub struct AddFriendRequest {
    pub other: String,
    pub key: IdempotencyKey,
}

pub async fn add_friend(
    body: AddFriendRequest,
    user_id: UserId,
    user_service: Arc<dyn UserService>,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let other_id: UserId = user_service
        .resolve_username(&body.other)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;

    let conversation = relationship_service
        .add_friend(user_id, other_id, body.key)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;

    Ok(warp::reply::json(&ApiResponse::ok(conversation)))
}

#[derive(Debug, Deserialize)]
pub struct ConversationHistoryQuery {
    pub conversation_id: ConversationId,
    pub page_size: PageSize,
    pub before: Option<String>,
}

pub async fn generate_conversation_history(
    query: ConversationHistoryQuery,
    user_id: UserId,
    conversation_service: Arc<dyn ConversationService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let page_size = query.page_size;
    let before = query
        .before
        .map(|s| s.parse::<OffsetCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;

    let history = conversation_service
        .get_history(user_id, query.conversation_id, page_size, before)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;

    let response = ApiResponse::ok(history);
    Ok(warp::reply::json(&response))
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

pub async fn refresh_token(
    body: RefreshTokenRequest,
    auth_service: Arc<dyn AuthService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let tokens = auth_service
        .refresh_token(&body.refresh_token)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(tokens)))
}

#[derive(Debug, Deserialize)]
pub struct RecentConversationsQuery {
    pub page_size: PageSize,
    pub after: Option<String>,
}

pub async fn recent_conversations(
    query: RecentConversationsQuery,
    user_id: UserId,
    conversation_service: Arc<dyn ConversationService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let after = query
        .after
        .map(|s| s.parse::<TimeCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;
    let result = conversation_service
        .recent_conversations(user_id, query.page_size, after)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(result)))
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub description: Option<String>,
    pub key: IdempotencyKey,
}

#[derive(Debug, Serialize)]
pub struct CreateGroupResponse {
    pub group_id: GroupId,
    pub conversation_id: ConversationId,
}

pub async fn create_group(
    body: CreateGroupRequest,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (group_id, conversation_id) = relationship_service
        .create_group(user_id, &body.name, body.description.as_deref(), body.key)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(CreateGroupResponse {
        group_id,
        conversation_id,
    })))
}

#[derive(Debug, Deserialize)]
pub struct GroupListQuery {
    pub page_size: PageSize,
    pub after: Option<String>,
}

pub async fn list_groups(
    query: GroupListQuery,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let after = query
        .after
        .map(|s| s.parse::<GroupCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;
    let groups = relationship_service
        .list_groups(user_id, query.page_size, after)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(groups)))
}

#[derive(Debug, Deserialize)]
pub struct GroupMembersQuery {
    pub page_size: PageSize,
    pub after: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendFriendRequestBody {
    pub other: String,
}

pub async fn send_friend_request(
    body: SendFriendRequestBody,
    user_id: UserId,
    user_service: Arc<dyn UserService>,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let other_id = user_service
        .resolve_username(&body.other)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    relationship_service
        .send_friend_request(user_id, other_id)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

#[derive(Debug, Deserialize)]
pub struct IncomingFriendRequestsQuery {
    pub page_size: PageSize,
    pub after: Option<String>,
}

pub async fn list_incoming_friend_requests(
    query: IncomingFriendRequestsQuery,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let after = query
        .after
        .map(|s| s.parse::<FriendRequestCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;
    let requests = relationship_service
        .list_incoming_friend_requests(user_id, query.page_size, after)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(requests)))
}

pub async fn accept_friend_request(
    requester_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let conversation_id = relationship_service
        .accept_friend_request(user_id, UserId(requester_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(conversation_id)))
}

pub async fn reject_friend_request(
    requester_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .reject_friend_request(user_id, UserId(requester_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

pub async fn remove_friend(
    other_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .remove_friend(user_id, UserId(other_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

// Group invitations
#[derive(Debug, Deserialize)]
pub struct InviteToGroupBody {
    pub group_id: uuid::Uuid,
    pub invitee: String,
}

pub async fn invite_to_group(
    body: InviteToGroupBody,
    user_id: UserId,
    user_service: Arc<dyn UserService>,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let invitee_id = user_service
        .resolve_username(&body.invitee)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    let invitation_id = relationship_service
        .invite_to_group_v2(GroupId(body.group_id), user_id, invitee_id)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(invitation_id)))
}

#[derive(Debug, Deserialize)]
pub struct GroupInvitationsQuery {
    pub page_size: PageSize,
    pub after: Option<String>,
}

pub async fn list_group_invitations(
    query: GroupInvitationsQuery,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let after = query
        .after
        .map(|s| s.parse::<GroupInvitationCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;
    let invitations = relationship_service
        .list_group_invitations(user_id, query.page_size, after)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(invitations)))
}

pub async fn accept_group_invitation(
    invitation_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .accept_group_invitation(user_id, GroupInvitationId(invitation_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

pub async fn reject_group_invitation(
    invitation_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .reject_group_invitation(user_id, GroupInvitationId(invitation_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

// Group join requests
#[derive(Debug, Deserialize)]
pub struct JoinGroupRequestBody {
    pub group_id: uuid::Uuid,
}

pub async fn request_to_join_group(
    body: JoinGroupRequestBody,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let request_id = relationship_service
        .request_to_join_group(user_id, GroupId(body.group_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(request_id)))
}

#[derive(Debug, Deserialize)]
pub struct JoinRequestsQuery {
    pub group_id: uuid::Uuid,
    pub page_size: PageSize,
    pub after: Option<String>,
}

pub async fn list_group_join_requests(
    group_id: uuid::Uuid,
    query: JoinRequestsQuery,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let after = query
        .after
        .map(|s| s.parse::<GroupJoinRequestCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;
    let requests = relationship_service
        .list_join_requests(user_id, GroupId(group_id), query.page_size, after)
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(requests)))
}

pub async fn accept_group_join_request(
    request_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .accept_join_request(user_id, GroupJoinRequestId(request_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

pub async fn reject_group_join_request(
    request_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .reject_join_request(user_id, GroupJoinRequestId(request_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

pub async fn get_user_profile(
    user_id_param: uuid::Uuid,
    _caller: UserId,
    user_service: Arc<dyn UserService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let profile = user_service
        .get_profile(UserId(user_id_param))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(profile)))
}

#[derive(Debug, Deserialize)]
pub struct UserSearchQuery {
    pub q: String,
    pub page_size: PageSize,
    pub after: Option<String>,
}

pub async fn search_users(
    query: UserSearchQuery,
    _caller: UserId,
    user_service: Arc<dyn UserService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let after = query
        .after
        .map(|s| s.parse::<UserCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;
    let profiles = user_service
        .search_users(&query.q, query.page_size, after)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(profiles)))
}

pub async fn list_group_members(
    group_id: uuid::Uuid,
    query: GroupMembersQuery,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let after = query
        .after
        .map(|s| s.parse::<MemberCursor>().map_err(ApiErrorCode::internal))
        .transpose()
        .map_err(reject::custom)?;
    let members = relationship_service
        .list_group_members(user_id, GroupId(group_id), query.page_size, after)
        .await
        .map_err(ApiErrorCode::internal)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(members)))
}

// Group management

pub async fn leave_group(
    group_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .leave_group(user_id, GroupId(group_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

pub async fn remove_group_member(
    group_id: uuid::Uuid,
    target_id: uuid::Uuid,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .remove_group_member(user_id, GroupId(group_id), UserId(target_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

#[derive(Debug, Deserialize)]
pub struct TransferOwnershipBody {
    pub new_owner_id: uuid::Uuid,
}

pub async fn transfer_group_ownership(
    group_id: uuid::Uuid,
    body: TransferOwnershipBody,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .transfer_group_ownership(user_id, GroupId(group_id), UserId(body.new_owner_id))
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroupInfoBody {
    pub name: String,
    pub description: Option<String>,
}

pub async fn update_group_info(
    group_id: uuid::Uuid,
    body: UpdateGroupInfoBody,
    user_id: UserId,
    relationship_service: Arc<dyn RelationshipService>,
) -> Result<impl warp::Reply, warp::Rejection> {
    relationship_service
        .update_group_info(
            user_id,
            GroupId(group_id),
            &body.name,
            body.description.as_deref(),
        )
        .await
        .map_err(ApiErrorCode::from)
        .map_err(reject::custom)?;
    Ok(warp::reply::json(&ApiResponse::ok(())))
}

pub async fn join_chat(
    socket: warp::ws::WebSocket,
    user_id: UserId,
    connection_acceptor: Arc<dyn ConnectionAcceptor>,
) {
    let (s2c, c2s) = socket.split();
    if let Err(e) = connection_acceptor
        .accept_connection(Box::new(s2c), Box::new(c2s), user_id)
        .await
    {
        error!("accepting connection: {}", e);
    }
}
