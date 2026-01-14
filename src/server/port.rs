use crate::domain_model::*;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use warp::ws::Message;

// region conn message

#[derive(Debug)]
pub enum ConnMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping,
    Pong,
    Close,
}

impl From<Message> for ConnMessage {
    fn from(message: Message) -> Self {
        if message.is_text() {
            ConnMessage::Text(message.to_str().unwrap_or_default().to_owned())
        } else if message.is_binary() {
            ConnMessage::Binary(message.as_bytes().to_vec())
        } else if message.is_ping() {
            ConnMessage::Ping
        } else if message.is_pong() {
            ConnMessage::Pong
        } else if message.is_close() {
            ConnMessage::Close
        } else {
            // NOTE: message converting happens in handshake,
            //       which is safe to panic
            unreachable!("Invalid message type")
        }
    }
}

impl From<ConnMessage> for Message {
    fn from(message: ConnMessage) -> Message {
        match message {
            ConnMessage::Text(t) => Message::text(t),
            ConnMessage::Binary(b) => Message::binary(b),
            ConnMessage::Ping => Message::ping(Vec::new()),
            ConnMessage::Pong => Message::pong(Vec::new()),
            ConnMessage::Close => Message::close(),
        }
    }
}

// endregion

// region conn sender

#[async_trait::async_trait]
pub trait ConnSender: Send + Sync {
    async fn send(&mut self, message: ConnMessage) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl ConnSender for SplitSink<warp::ws::WebSocket, Message> {
    async fn send(&mut self, message: ConnMessage) -> anyhow::Result<()> {
        SinkExt::send(&mut self, Message::from(message)).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ConnSender for Sender<ConnMessage> {
    async fn send(&mut self, message: ConnMessage) -> anyhow::Result<()> {
        Sender::<ConnMessage>::send(self, message).await?;
        Ok(())
    }
}

// endregion

// region conn receiver

#[async_trait::async_trait]
pub trait ConnReceiver: Send + Sync {
    async fn next(&mut self) -> Option<anyhow::Result<ConnMessage>>;
}

#[async_trait::async_trait]
impl ConnReceiver for SplitStream<warp::ws::WebSocket> {
    async fn next(&mut self) -> Option<anyhow::Result<ConnMessage>> {
        StreamExt::next(&mut self)
            .await
            .map(|result| result.map(ConnMessage::from).map_err(anyhow::Error::from))
    }
}

#[async_trait::async_trait]
impl ConnReceiver for Receiver<ConnMessage> {
    async fn next(&mut self) -> Option<anyhow::Result<ConnMessage>> {
        Some(Ok(Receiver::<ConnMessage>::recv(&mut *self).await?))
    }
}

// endregion

#[derive(Debug)]
pub struct WsMessage(pub String);

#[async_trait::async_trait]
pub trait ConnectionAcceptor: Send + Sync {
    async fn accept_connection(
        &self,
        s2c_channel: Box<dyn ConnSender>,
        c2s_channel: Box<dyn ConnReceiver>,
        user_id: UserId,
    ) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait OutboundQueue: Send + Sync {
    async fn enqueue(&self, receiver: UserId, event: &S2CEvent) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish(&self, topic: &str, key: &[u8], payload: &[u8]) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait EventConsumer: Send + Sync {
    async fn run(
        &self,
        consumer_group_id: &str,
        topics: &[&str],
        handler: Arc<dyn EventHandler>,
    ) -> anyhow::Result<()>;
}

pub enum HandleOutcome {
    Commit,
    Retry,
    SkipCommit,
}

#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, payload: &[u8]) -> anyhow::Result<HandleOutcome>;
}
