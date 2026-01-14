use crate::domain_port::*;
use crate::server::EventPublisher;
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct Notifier {
    tx_manager: Arc<dyn TxManager>,
    outbox_repo: Arc<dyn OutboxRepo>,
    event_publisher: Arc<dyn EventPublisher>,
    topic: String,
    cancellation_token: CancellationToken,
}

impl Notifier {
    pub fn new(
        tx_manager: Arc<dyn TxManager>,
        outbox_repo: Arc<dyn OutboxRepo>,
        event_publisher: Arc<dyn EventPublisher>,
        topic: &str,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            tx_manager,
            outbox_repo,
            event_publisher,
            topic: topic.to_owned(),
            cancellation_token,
        }
    }

    fn build_envelope(
        receivers_json: &serde_json::Value,
        payload_json: &serde_json::Value,
    ) -> anyhow::Result<Vec<u8>> {
        let envelope = json!({
            "receivers": receivers_json,
            "body": payload_json,
        });

        Ok(serde_json::to_vec(&envelope)?)
    }

    async fn tick_once(&self) -> anyhow::Result<()> {
        let mut tx = self.tx_manager.begin().await?;

        let now = Utc::now();
        let batch = self
            .outbox_repo
            .claim_ready_batch_in_tx(&mut *tx, now, 256)
            .await?;

        if batch.is_empty() {
            tx.commit().await?;
            tokio::time::sleep(Duration::from_millis(200)).await;
            return Ok(());
        }

        for event in &batch {
            let key = match event.partition_key {
                Some(key) => key,
                None => event.event_id.0,
            };
            let payload = Self::build_envelope(&event.receivers_json, &event.payload_json)?;

            match self
                .event_publisher
                .publish(&self.topic, key.as_bytes(), &payload)
                .await
            {
                Ok(()) => {
                    self.outbox_repo
                        .mark_delivered_in_tx(&mut *tx, event.event_id, Utc::now())
                        .await?;
                }
                Err(e) => {
                    // backoff
                    let next = Utc::now() + chrono::Duration::seconds(2);
                    self.outbox_repo
                        .reschedule_in_tx(&mut *tx, event.event_id, next, &format!("{e:#}"))
                        .await?;
                }
            }
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                biased;
                _ = self.cancellation_token.cancelled() => {
                    tracing::info!("Notifier shutting down...");
                    break;
                }
                result = self.tick_once() => {
                    if let Err(e) = result {
                        tracing::error!("Notifier error: {:#?}", e);
                    }
                }
            }
        }
        Ok(())
    }
}
