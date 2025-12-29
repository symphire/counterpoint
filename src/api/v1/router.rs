use super::error::*;
use super::handler;
use crate::auth::*;
use crate::chat::ChatService;
use crate::domain::AuthService;
use crate::domain::UserId;
use crate::server::*;
use std::convert::Infallible;
use std::sync::Arc;
use warp::{http, reject, Filter};
use crate::api::v1::handler::{ConversationHistoryQuery, FriendListQuery};

pub fn routes(
    server: Arc<Server>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    // TODO: need a timeout
    let captcha = warp::get()
        .and(warp::path("captcha"))
        .and(warp::path::end())
        .and(with(server.captcha_service.clone()))
        .and_then(handler::generate_captcha);

    let login = warp::post()
        .and(warp::path("login"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(with(server.auth_service.clone()))
        .and(with(server.captcha_service.clone()))
        .and_then(handler::login);

    let signup = warp::post()
        .and(warp::path("signup"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(with(server.auth_service.clone()))
        .and(with(server.captcha_service.clone()))
        .and_then(handler::signup);

    let friend_list = warp::get()
        .and(warp::path("friend_list"))
        .and(warp::path::end())
        .and(warp::query::<FriendListQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::generate_friend_list);

    let add_friend = warp::post()
        .and(warp::path("add_friend"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.user_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::add_friend);
    
    let conversation_history = warp::get()
        .and(warp::path("conversation_history"))
        .and(warp::path::end())
        .and(warp::query::<ConversationHistoryQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.conversation_service.clone()))
        .and_then(handler::generate_conversation_history);

    let chat = warp::get()
        .and(warp::path("chat"))
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(warp::ws())
        .and(with(server.chat_service.clone()))
        .and(with(server.connection_acceptor.clone()))
        .map(
            |user_id: UserId,
             ws: warp::ws::Ws,
             chat_service: Arc<dyn ChatService>,
             connection_acceptor: Arc<dyn ConnectionAcceptor>| {
                ws.on_upgrade(move |socket| {
                    handler::join_chat(socket, user_id, chat_service, connection_acceptor)
                })
            },
        );

    captcha.or(login).or(signup).or(friend_list).or(add_friend).or(conversation_history).or(chat)
}

fn with<ServiceType>(
    service: Arc<ServiceType>,
) -> impl Filter<Extract = (Arc<ServiceType>,), Error = Infallible> + Clone
where
    ServiceType: Send + Sync + ?Sized,
{
    warp::any().map(move || service.clone())
}

fn with_verification(
    auth_service: Arc<dyn AuthService>,
) -> impl Filter<Extract = (UserId,), Error = warp::Rejection> + Clone {
    warp::header::<String>(http::header::AUTHORIZATION.as_ref()).and_then(move |token: String| {
        let auth_service = auth_service.clone();
        async move {
            if let Some(token) = token.strip_prefix("Bearer ") {
                let user_id = auth_service
                    .verify_token(token)
                    .await
                    .map_err(ApiErrorCode::from)
                    .map_err(reject::custom)?;
                Ok(user_id)
            } else {
                Err(reject::custom(ApiErrorCode::InvalidToken))
            }
        }
    })
}
