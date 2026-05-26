use crate::api::v1::handler::ApiResponse;
use crate::application_port::*;
use serde::Serialize;
use std::convert::Infallible;
use thiserror::Error;
use tracing::warn;
use warp::http::StatusCode;
use warp::{Rejection, reject};

impl From<RelationError> for ApiErrorCode {
    fn from(error: RelationError) -> Self {
        match error {
            RelationError::UserNotFound
            | RelationError::GroupNotFound
            | RelationError::FriendRequestNotFound
            | RelationError::InvitationNotFound
            | RelationError::JoinRequestNotFound => ApiErrorCode::NotFound,
            RelationError::AlreadyFriends
            | RelationError::FriendRequestExists
            | RelationError::AlreadyMember => ApiErrorCode::Conflict,
            RelationError::NotMember | RelationError::NotOwner => ApiErrorCode::Forbidden,
            e => ApiErrorCode::internal(e),
        }
    }
}

pub async fn recover_error(err: Rejection) -> Result<impl warp::Reply, Infallible> {
    if let Some(err) = err.find::<ApiErrorCode>() {
        let json = warp::reply::json(&ApiResponse::<()>::err(err.clone(), err.to_string()));
        Ok(warp::reply::with_status(json, StatusCode::OK))
    } else {
        let json = warp::reply::json(&ApiResponse::<()> {
            success: false,
            data: None,
            error: Some(ApiError {
                code: ApiErrorCode::InternalError,
                message: format!("Unhandled error: {:?}", err),
            }),
        });
        Ok(warp::reply::with_status(
            json,
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Error, Serialize)]
pub enum ApiErrorCode {
    #[error("Invalid captcha ID or answer")]
    InvalidCaptcha,
    #[error("Invalid username or password")]
    InvalidCredentials,
    #[error("Username already taken")]
    UsernameTaken,
    #[error("Token is not valid")]
    InvalidToken,
    #[error("Not found")]
    NotFound,
    #[error("Conflict")]
    Conflict,
    #[error("Forbidden")]
    Forbidden,
    #[error("Internal error")]
    InternalError,
}

impl ApiErrorCode {
    pub fn internal<E: std::fmt::Display>(error: E) -> ApiErrorCode {
        warn!("Internal error: {}", error);
        ApiErrorCode::InternalError
    }
}

impl reject::Reject for ApiErrorCode {}

impl From<CaptchaError> for ApiErrorCode {
    fn from(error: CaptchaError) -> Self {
        match error {
            CaptchaError::Incorrect { .. } => ApiErrorCode::InvalidCaptcha,
            CaptchaError::NotFoundOrExpired => ApiErrorCode::InvalidCaptcha,
            CaptchaError::Store(e) => ApiErrorCode::internal(e),
            CaptchaError::InternalError(e) => ApiErrorCode::internal(e),
        }
    }
}

impl From<AuthError> for ApiErrorCode {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::InvalidCredentials => ApiErrorCode::InvalidCredentials,
            AuthError::UserExists => ApiErrorCode::UsernameTaken,
            AuthError::UserNotFound => ApiErrorCode::NotFound,
            AuthError::TokenInvalid | AuthError::TokenExpired => ApiErrorCode::InvalidToken,
            AuthError::InternalError(e) => ApiErrorCode::internal(e),
            _ => ApiErrorCode::InternalError,
        }
    }
}
