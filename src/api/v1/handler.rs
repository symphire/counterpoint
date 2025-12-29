use super::error::*;
use crate::auth::*;
use crate::chat::ChatService;
use crate::domain::{ConversationId, ConversationService, IdempotencyKey, OffsetCursor, UserId, UserService};
use crate::domain::{
    AuthService, AuthTokens, CaptchaId, CaptchaService, FriendCursor, LoginInput, PageSize,
    RelationshipService, SignupInput, ValidationInput,
};
use crate::logger::*;
use crate::server::{ConnSender, ConnectionAcceptor};
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, TryFutureExt};
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
    let after = query.after
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
    let before = query.before
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

pub async fn join_chat(
    socket: warp::ws::WebSocket,
    user_id: UserId,
    chat_service: Arc<dyn ChatService>,
    connection_acceptor: Arc<dyn ConnectionAcceptor>,
) {
    let (s2c, c2s) = socket.split();
    if let Err(e) = connection_acceptor
        .accept_connection(Box::new(s2c), Box::new(c2s), user_id)
        .await
    {
        error!("accepting connection: {}", e);
    }
    // if let Err(e) = chat_service.join_chat(to_user, from_user, user_id).await {
    //     error!("Error joining chat: {}", e);
    // }
}
