use crate::application_port::*;
use crate::domain_model::*;
use crate::server::*;
use anyhow::anyhow;
use dashmap::DashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{Notify, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const MAILBOX_CAP: usize = 256;

pub struct ActorConfig {
    pub max_inflight_messages: usize,
    pub max_inflight_results: usize,
    pub max_worker_timeout: u64,
}

pub struct ClientRecord {
    pub user_id: UserId,
    pub control: Sender<ConnMessage>,
    pub mailbox: Sender<ConnMessage>,
    pub actor_handle: Mutex<Option<JoinHandle<()>>>,
    pub cancellation_token: CancellationToken,
}

pub struct ServiceRegistry {
    pub conversation_service: Arc<dyn ConversationService>,
}

pub struct SessionHub {
    online_users: Arc<DashMap<UserId, ClientRecord>>,
    services: Arc<ServiceRegistry>,
}

impl SessionHub {
    pub fn new(services: Arc<ServiceRegistry>) -> Self {
        let online_users = Arc::new(DashMap::new());

        Self {
            online_users,
            services,
        }
    }

    pub async fn shutdown(&self) {
        tracing::info!("SessionHub shutting down...");

        for entry in self.online_users.iter() {
            entry.cancellation_token.cancel();
        }

        let mut handles = Vec::new();
        for entry in self.online_users.iter() {
            if let Ok(mut lock) = entry.actor_handle.lock() {
                if let Some(handle) = lock.take() {
                    handles.push(handle);
                }
            }
        }

        for handle in handles {
            let _ = handle.await;
        }

        tracing::info!("All SessionHub actors shut down.");
    }
}

// region connection acceptor

#[async_trait::async_trait]
impl ConnectionAcceptor for SessionHub {
    async fn accept_connection(
        &self,
        s2c_channel: Box<dyn ConnSender>,
        c2s_channel: Box<dyn ConnReceiver>,
        user_id: UserId,
    ) -> anyhow::Result<()> {
        let config = ActorConfig {
            max_inflight_messages: 64,
            max_inflight_results: 1024,
            max_worker_timeout: 1000,
        };

        let services = self.services.clone();

        let actor_cancel = CancellationToken::new();

        let (sender_control_tx, sender_control_rx) = tokio::sync::mpsc::channel(MAILBOX_CAP);
        let (sender_buffer_tx, sender_buffer_rx) = tokio::sync::mpsc::channel(MAILBOX_CAP);

        let notify = Arc::new(Notify::new());
        let actor_handle = tokio::spawn(client_actor(
            user_id,
            s2c_channel,
            c2s_channel,
            sender_control_tx.clone(),
            sender_control_rx,
            sender_buffer_tx.clone(),
            sender_buffer_rx,
            services,
            config,
            actor_cancel.clone(),
            notify.clone(),
            self.online_users.clone(),
        ));

        let new_user = ClientRecord {
            user_id,
            control: sender_control_tx,
            mailbox: sender_buffer_tx,
            actor_handle: Mutex::new(Some(actor_handle)),
            cancellation_token: actor_cancel,
        };
        self.online_users.insert(user_id, new_user);
        notify.notify_one();

        Ok(())
    }
}

async fn client_actor(
    user_id: UserId,
    s2c_channel: Box<dyn ConnSender>,
    c2s_channel: Box<dyn ConnReceiver>,
    sender_control_tx: Sender<ConnMessage>,
    sender_control_rx: Receiver<ConnMessage>,
    sender_data_tx: Sender<ConnMessage>,
    sender_data_rx: Receiver<ConnMessage>,
    services: Arc<ServiceRegistry>,
    config: ActorConfig,
    actor_cancel: CancellationToken,
    notify: Arc<Notify>,
    online_users: Arc<DashMap<UserId, ClientRecord>>,
) {
    notify.notified().await;
    tracing::info!("ClientActor [{}] starting", user_id);

    let sender_token = actor_cancel.clone();
    let sender_handle = tokio::spawn(outbound_sender(
        s2c_channel,
        sender_control_rx,
        sender_data_rx,
        sender_token,
    ));

    let receiver_token = actor_cancel.clone();
    let receiver_handle = tokio::spawn(inbound_receiver(
        user_id,
        c2s_channel,
        sender_control_tx,
        sender_data_tx,
        services,
        config,
        receiver_token,
    ));

    let _ = tokio::select! {
        res = sender_handle => {
            tracing::warn!("Sender task ended first ({:?}): {:?}", user_id, res);
        },
        res = receiver_handle => {
            tracing::warn!("Receiver task ended first ({:?}): {:?}", user_id, res);
        }
    };
    online_users.remove(&user_id);
    tracing::debug!("online_users: {}", online_users.len());
}

async fn outbound_sender(
    mut s2c_channel: Box<dyn ConnSender>,
    mut sender_control_rx: Receiver<ConnMessage>,
    mut sender_data_rx: Receiver<ConnMessage>,
    actor_cancel: CancellationToken,
) {
    while let Some(msg) = tokio::select! {
        biased;
        _ = actor_cancel.cancelled() => None,
        m = sender_control_rx.recv() => m,
        m = sender_data_rx.recv() => m,
    } {
        tracing::trace!("outbound_sender: {:?}", msg);
        if s2c_channel.send(msg).await.is_err() {
            tracing::trace!("outbound_sender shutting down");
            actor_cancel.cancel();
            break;
        }
    }
}

async fn inbound_receiver(
    user_id: UserId,
    mut c2s_channel: Box<dyn ConnReceiver>,
    sender_control_tx: Sender<ConnMessage>,
    sender_data_tx: Sender<ConnMessage>,
    services: Arc<ServiceRegistry>,
    config: ActorConfig,
    actor_cancel: CancellationToken,
) {
    let worker_sem = Arc::new(Semaphore::new(config.max_inflight_messages));
    let join_sem = Arc::new(Semaphore::new(config.max_inflight_results));

    let mut task_set = tokio::task::JoinSet::new();

    loop {
        let sender_control_tx = sender_control_tx.clone();
        let services = services.clone();
        let actor_cancel = actor_cancel.clone();

        tokio::select! {
            biased;

            _ = actor_cancel.cancelled() => {
                tracing::info!("ClientActor [{}] shutdown by cancel", user_id);
                break;
            },

            maybe_message = c2s_channel.next() => {
                let result = match maybe_message {
                    Some(result) => result,
                    None => break,  // connection closed
                };

                let conn_msg = match result {
                    Ok(m) => m,
                    Err(_) => break,  // low level error
                };

                let permit = match worker_sem.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => {
                        tracing::warn!("Client [{}] is throttled", user_id);
                        let _ = sender_control_tx.send(ConnMessage::Text(String::from("Too many messages"))).await;
                        continue;
                    }
                };

                let join_permit = match join_sem.try_acquire() {
                    Ok(p) => p,
                    Err(_) => {
                        tracing::warn!("Client [{}] join-backlog limit reached", user_id);
                        continue;
                    }
                };
                join_permit.forget();

                task_set.spawn(async move {
                    let _permit_guard = permit;
                    let fut = handle_incoming_message(
                        user_id,
                        conn_msg,
                        sender_control_tx,
                        services,
                        actor_cancel.clone(),
                    );
                    let result = tokio::time::timeout(
                        Duration::from_secs(config.max_worker_timeout),
                        fut,
                    ).await;
                    if let Err(_) = result {
                        tracing::warn!("Worker timeout for client [{}]", user_id);
                    }
                });
            }

            Some(join_result) = task_set.join_next() => {
                if let Err(e) = join_result {
                    tracing::error!("worker panicked: {e}");
                }
                join_sem.add_permits(1);
            }
        }
    }

    actor_cancel.cancel();
    while task_set.join_next().await.is_some() {}
    tracing::info!("ClientActor [{}] shutting down", user_id);
}

async fn handle_incoming_message(
    user_id: UserId,
    conn_msg: ConnMessage,
    sender_control_tx: Sender<ConnMessage>,
    services: Arc<ServiceRegistry>,
    actor_cancel: CancellationToken,
) -> anyhow::Result<()> {
    match conn_msg {
        ConnMessage::Text(t) => {
            if let Ok(request) = serde_json::from_str::<C2SCommand>(&t) {
                let sender = user_id;
                let result = match request {
                    C2SCommand::ChatMessageSend(data) => {
                        send_message(sender, data, services.conversation_service.clone()).await
                    }
                };

                match result {
                    Ok(record) => {
                        let ack = S2CEvent::ChatMessageACK(ChatMessageACK {
                            conversation_id: record.conversation_id,
                            message_id: record.message_id,
                            message_offset: record.message_offset,
                            created_at: record.created_at,
                        });
                        let _ = sender_control_tx
                            .send(ConnMessage::Text(serde_json::to_string(&ack)?))
                            .await;
                        Ok(())
                    }
                    Err(e) => {
                        tracing::error!("Failed to send message: {e}");
                        Err(anyhow!(e))
                    }
                }
            } else {
                tracing::error!("failed to deserialize message: {}", t);
                let result = sender_control_tx
                    .send(ConnMessage::Text("malformed message".to_owned()))
                    .await;
                match result {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!(e)),
                }
            }
        }
        ConnMessage::Binary(_) => {
            tracing::error!("unexpected binary message from [{}]", user_id);
            Ok(())
        }
        ConnMessage::Ping => {
            let _ = sender_control_tx.send(ConnMessage::Pong).await?;
            Ok(())
        }
        ConnMessage::Pong => {
            tracing::error!("unexpected pong from [{}]", user_id);
            Ok(())
        }
        ConnMessage::Close => {
            actor_cancel.cancel();
            Ok(())
        }
    }
}

/// handler

async fn send_message(
    sender: UserId,
    data: ChatMessageSend,
    conversation_service: Arc<dyn ConversationService>,
) -> anyhow::Result<MessageRecord> {
    let record = conversation_service
        .send_message(
            data.conversation_id,
            sender,
            data.content.as_str(),
            data.message_id,
        )
        .await
        .map_err(|e| anyhow::anyhow!("failed to send chat message: {}", e))?;
    Ok(record)
}

// endregion

// region outbound queue

#[async_trait::async_trait]
impl OutboundQueue for SessionHub {
    async fn enqueue(&self, receiver: UserId, event: &S2CEvent) -> anyhow::Result<()> {
        if let Some(record) = self.online_users.get(&receiver) {
            let message = serde_json::to_string(event)?;
            match record.mailbox.try_send(ConnMessage::Text(message)) {
                Ok(_) => Ok(()),
                Err(TrySendError::Full(..)) => Err(anyhow!("backpressure retry")),
                Err(e) => Err(anyhow!("failed to enqueue message: {e}")),
            }
        } else {
            Err(anyhow::anyhow!("user {} not connected", receiver))
        }
    }
}

// endregion
