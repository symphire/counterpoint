use crate::server::{EventConsumer, EventHandler, HandleOutcome};
use futures_util::StreamExt;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::{ClientConfig, Message};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct KafkaConsumer {
    bootstrap_server: String,
    client_id: String,
    cancellation_token: CancellationToken,
}

impl KafkaConsumer {
    pub fn new(
        bootstrap_server: &str,
        client_id: &str,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            bootstrap_server: bootstrap_server.to_string(),
            client_id: client_id.to_string(),
            cancellation_token,
        }
    }

    async fn ensure_topics(bootstrap: &str, topics: &[&str]) -> anyhow::Result<()> {
        let admin: AdminClient<_> = ClientConfig::new()
            .set("bootstrap.servers", bootstrap)
            .create()?;

        let new_topics: Vec<_> = topics
            .iter()
            .map(|t| NewTopic::new(t, 1, TopicReplication::Fixed(1)))
            .collect();

        let _ = admin
            .create_topics(&new_topics, &AdminOptions::new())
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl EventConsumer for KafkaConsumer {
    async fn run(
        &self,
        consumer_group_id: &str,
        topics: &[&str],
        handler: Arc<dyn EventHandler>,
    ) -> anyhow::Result<()> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &self.bootstrap_server)
            .set("client.id", &self.client_id)
            .set("group.id", consumer_group_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .create()?;

        Self::ensure_topics(&self.bootstrap_server, topics).await?;
        consumer.subscribe(topics)?;

        let mut stream = consumer.stream();

        loop {
            let result = tokio::select! {
                biased;
                _ = self.cancellation_token.cancelled() => {
                    tracing::info!("Kafka consumer shutting down...");
                    break;
                }
                msg = stream.next() => msg,
            };

            let Some(message) = result else {
                tracing::error!("Kafka consumer stream terminated");
                break;
            };

            match message {
                Err(e) => {
                    // broker hiccup
                    tracing::warn!(error = ?e, "consumer poll error");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                Ok(m) => {
                    let topic = m.topic();
                    let key = m.key().unwrap_or(&[]);
                    let payload = m.payload().unwrap_or(&[]);

                    match handler.handle(payload).await {
                        Ok(HandleOutcome::Commit | HandleOutcome::SkipCommit) => {
                            if let Err(e) =
                                consumer.commit_message(&m, rdkafka::consumer::CommitMode::Async)
                            {
                                tracing::warn!(error = ?e, "commit failed but ignored");
                            }
                        }
                        Ok(HandleOutcome::Retry) => {
                            // do nothing, retry on next poll
                            // add a small delay to avoid hot-loop on poison messages
                            // TODO: add a DLQ for poison messages
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                        Err(e) => {
                            tracing::error!(error = ?e, "handler error; retrying");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }

        consumer.unsubscribe();

        Ok(())
    }
}
