use super::error::*;
use super::handler;
use crate::api::v1::handler::{
    ConversationHistoryQuery, FriendListQuery, GroupInvitationsQuery, GroupListQuery,
    GroupMembersQuery, IncomingFriendRequestsQuery, JoinRequestsQuery, RecentConversationsQuery,
    TransferOwnershipBody, UpdateGroupInfoBody, UserSearchQuery,
};
use crate::application_port::*;
use crate::domain_model::UserId;
use crate::server::*;
use std::convert::Infallible;
use std::sync::Arc;
use warp::{Filter, http, reject};

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
        .and(with(server.connection_acceptor.clone()))
        .map(
            |user_id: UserId,
             ws: warp::ws::Ws,
             connection_acceptor: Arc<dyn ConnectionAcceptor>| {
                ws.on_upgrade(move |socket| {
                    handler::join_chat(socket, user_id, connection_acceptor)
                })
            },
        );

    let auth_refresh = warp::post()
        .and(warp::path("auth"))
        .and(warp::path("refresh"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(with(server.auth_service.clone()))
        .and_then(handler::refresh_token);

    let recent_conversations = warp::get()
        .and(warp::path("me"))
        .and(warp::path("conversations"))
        .and(warp::path::end())
        .and(warp::query::<RecentConversationsQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.conversation_service.clone()))
        .and_then(handler::recent_conversations);

    let create_group = warp::post()
        .and(warp::path("groups"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::create_group);

    let list_groups = warp::get()
        .and(warp::path("me"))
        .and(warp::path("groups"))
        .and(warp::path::end())
        .and(warp::query::<GroupListQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::list_groups);

    let list_group_members = warp::get()
        .and(warp::path("groups"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("members"))
        .and(warp::path::end())
        .and(warp::query::<GroupMembersQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::list_group_members);

    let send_friend_request = warp::post()
        .and(warp::path("friend-requests"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.user_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::send_friend_request);

    let list_incoming_friend_requests = warp::get()
        .and(warp::path("me"))
        .and(warp::path("friend-requests"))
        .and(warp::path("incoming"))
        .and(warp::path::end())
        .and(warp::query::<IncomingFriendRequestsQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::list_incoming_friend_requests);

    let accept_friend_request = warp::post()
        .and(warp::path("friend-requests"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("accept"))
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::accept_friend_request);

    let reject_friend_request = warp::post()
        .and(warp::path("friend-requests"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("reject"))
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::reject_friend_request);

    let remove_friend = warp::delete()
        .and(warp::path("friends"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::remove_friend);

    let get_user_profile = warp::get()
        .and(warp::path("users"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.user_service.clone()))
        .and_then(handler::get_user_profile);

    let search_users = warp::get()
        .and(warp::path("users"))
        .and(warp::path("search"))
        .and(warp::path::end())
        .and(warp::query::<UserSearchQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.user_service.clone()))
        .and_then(handler::search_users);

    let invite_to_group = warp::post()
        .and(warp::path("group-invitations"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.user_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::invite_to_group);

    let list_group_invitations = warp::get()
        .and(warp::path("me"))
        .and(warp::path("group-invitations"))
        .and(warp::path::end())
        .and(warp::query::<GroupInvitationsQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::list_group_invitations);

    let accept_group_invitation = warp::post()
        .and(warp::path("group-invitations"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("accept"))
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::accept_group_invitation);

    let reject_group_invitation = warp::post()
        .and(warp::path("group-invitations"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("reject"))
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::reject_group_invitation);

    let request_to_join_group = warp::post()
        .and(warp::path("group-join-requests"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::request_to_join_group);

    let list_group_join_requests = warp::get()
        .and(warp::path("groups"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("join-requests"))
        .and(warp::path::end())
        .and(warp::query::<JoinRequestsQuery>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::list_group_join_requests);

    let accept_group_join_request = warp::post()
        .and(warp::path("group-join-requests"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("accept"))
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::accept_group_join_request);

    let reject_group_join_request = warp::post()
        .and(warp::path("group-join-requests"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("reject"))
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::reject_group_join_request);

    let leave_group = warp::post()
        .and(warp::path("groups"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("leave"))
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::leave_group);

    let remove_group_member = warp::delete()
        .and(warp::path("groups"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("members"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path::end())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::remove_group_member);

    let transfer_group_ownership = warp::post()
        .and(warp::path("groups"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path("transfer-ownership"))
        .and(warp::path::end())
        .and(warp::body::json::<TransferOwnershipBody>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::transfer_group_ownership);

    let update_group_info = warp::patch()
        .and(warp::path("groups"))
        .and(warp::path::param::<uuid::Uuid>())
        .and(warp::path::end())
        .and(warp::body::json::<UpdateGroupInfoBody>())
        .and(with_verification(server.auth_service.clone()))
        .and(with(server.relationship_service.clone()))
        .and_then(handler::update_group_info);

    captcha
        .or(login)
        .or(signup)
        .or(auth_refresh)
        .or(friend_list)
        .or(add_friend)
        .or(recent_conversations)
        .or(conversation_history)
        .or(create_group)
        .or(list_groups)
        .or(list_group_members)
        .or(send_friend_request)
        .or(list_incoming_friend_requests)
        .or(accept_friend_request)
        .or(reject_friend_request)
        .or(remove_friend)
        .or(invite_to_group)
        .or(list_group_invitations)
        .or(accept_group_invitation)
        .or(reject_group_invitation)
        .or(request_to_join_group)
        .or(list_group_join_requests)
        .or(accept_group_join_request)
        .or(reject_group_join_request)
        .or(search_users)
        .or(get_user_profile)
        .or(leave_group)
        .or(remove_group_member)
        .or(transfer_group_ownership)
        .or(update_group_info)
        .or(chat)
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
