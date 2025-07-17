use crate::chat::*;
use crate::domain::UserId;
use crate::logger::*;
use crate::user::*;
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;
use warp::ws::{Message, WebSocket};

pub struct ClientRecord {
    pub user_id: UserId,
    pub to_sender: UnboundedSender<Message>,
    pub watcher_handle: JoinHandle<Result<()>>,
}

struct WithSender<T> {
    pub sender: UserId,
    pub body: T,
}

impl Debug for FakeChatService {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeChatService")
            .field("online_users", &self.online_users.len())
            .finish()
    }
}

pub struct FakeChatService {
    user_service: Arc<dyn UserService>,
    online_users: Arc<DashMap<UserId, ClientRecord>>,
    to_dispatcher: UnboundedSender<WithSender<ClientToServer>>,
    dispatcher_handle: JoinHandle<()>,
}

async fn dispatcher(
    mut from_receiver: UnboundedReceiver<WithSender<ClientToServer>>,
    online_users: Arc<DashMap<UserId, ClientRecord>>,
    user_service: Arc<dyn UserService>,
) {
    while let Some(message) = from_receiver.recv().await {
        if let Err(e) = dispatch(online_users.clone(), user_service.clone(), message).await {
            warn!("Error dispatching message: {}", e);
        }
    }
}

async fn dispatch(
    online_users: Arc<DashMap<UserId, ClientRecord>>,
    user_service: Arc<dyn UserService>,
    message: WithSender<ClientToServer>,
) -> Result<()> {
    let sender = message.sender;
    let message = match message.body {
        ClientToServer::Send(message) => message,
        _ => return Ok(()),
    };
    let content = message.content;

    let ack_message = ServerToClient::ACK(ACK {
        message_seq: message.message_seq,
    });
    let message = serde_json::to_string(&ack_message)?;
    let client_record = online_users.get(&sender).ok_or(anyhow!("Sender not online: {:?}", sender))?;
    client_record.to_sender.send(Message::text(message))?;

    let recipients = user_service.get_receiver(&sender, &content.conversation_id).await?;

    let distribute_message = ServerToClient::Distribute(DistributeMessage {
        sender,
        content,
    });
    let message = serde_json::to_string(&distribute_message)?;
    for recipient in recipients {
        let client_record = online_users.get(&recipient).ok_or(anyhow!("User not online: {:?}", recipient))?;
        client_record.to_sender.send(Message::text(message.clone()))?;
    }
    Ok(())
}

impl FakeChatService {
    pub fn new(user_service: Arc<dyn UserService>) -> Self {
        let (to_dispatcher, from_receiver) = unbounded_channel();
        let online_users = Arc::new(DashMap::new());
        let dispatcher_handle = tokio::spawn(dispatcher(
            from_receiver,
            online_users.clone(),
            user_service.clone(),
        ));

        Self {
            user_service,
            online_users,
            to_dispatcher,
            dispatcher_handle,
        }
    }
}

#[async_trait::async_trait]
impl ChatService for FakeChatService {
    async fn join_chat(
        &self,
        mut to_user: SplitSink<WebSocket, Message>,
        mut from_user: SplitStream<WebSocket>,
        user_id: UserId,
    ) -> Result<(), anyhow::Error> {
        let (to_sender, from_dispatcher) = unbounded_channel();
        let sender_handle = tokio::spawn(sender(from_dispatcher, to_user));
        let receiver_handle = tokio::spawn(receiver(
            from_user,
            user_id.clone(),
            to_sender.clone(),
            self.to_dispatcher.clone(),
        ));
        let watcher_handle = tokio::spawn(watcher(
            sender_handle,
            receiver_handle,
            user_id.clone(),
            self.online_users.clone(),
        ));

        let user_id_clone = user_id.clone();
        let new_user = ClientRecord {
            user_id,
            to_sender,
            watcher_handle,
        };
        self.online_users.insert(user_id_clone, new_user);
        debug!("online_users: {}", self.online_users.len());

        Ok(())
    }
}

// region join_chat helpers

async fn sender(
    mut from_dispatcher: UnboundedReceiver<Message>,
    mut to_user: SplitSink<WebSocket, Message>,
) {
    while let Some(message) = from_dispatcher.recv().await {
        if let Err(_) = to_user.send(message).await {
            break;
        }
    }
}

async fn receiver(
    mut from_user: SplitStream<WebSocket>,
    user_id: UserId,
    to_sender: UnboundedSender<Message>,
    to_dispatcher: UnboundedSender<WithSender<ClientToServer>>,
) {
    while let Some(result) = from_user.next().await {
        let message = match result {
            Ok(message) => message,
            Err(_) => break,
        };
        // if message.is_text() {
        //     warn!("Received a text message from user: {:?}", user_id);
        // }
        if message.is_close() {
            break;
        }
        // if message.is_binary() {
        //     warn!("Received a binary message from user: {:?}", user_id);
        // }

        if let Err(e) = handle_recv_message(&to_sender, &user_id, &to_dispatcher, &message).await {
            warn!("Failed to receive message: {}", e);
        }
    }
}

async fn handle_recv_message(
    to_sender: &UnboundedSender<Message>,
    user_id: &UserId,
    to_dispatcher: &UnboundedSender<WithSender<ClientToServer>>,
    message: &Message,
) -> Result<()> {
    if message.is_ping() {
        let _ = to_sender.send(Message::pong(vec![]));
        Ok(())
    } else if message.is_text() {
        let text = message.to_str().unwrap_or_default();
        let body = serde_json::from_str::<ClientToServer>(&text)?;
        let protocol_message = WithSender {
            sender: user_id.clone(),
            body,
        };
        let _ = to_dispatcher.send(protocol_message);
        Ok(())
    } else {
        Err(anyhow!("Unexpected message type"))
    }
}

async fn watcher(
    sender_handle: JoinHandle<()>,
    receiver_handle: JoinHandle<()>,
    user_id: UserId,
    online_users: Arc<DashMap<UserId, ClientRecord>>,
) -> Result<()> {
    let _ = tokio::select! {
        res = sender_handle => {
            warn!("Sender task ended: {:?}", res);
        },
        res = receiver_handle => {
            warn!("Receiver task ended: {:?}", res);
        },
    };
    online_users.remove(&user_id);
    debug!("online_users: {}", online_users.len());
    Ok(())
}

// endregion
