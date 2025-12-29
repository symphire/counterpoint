use std::time::Duration;
use rdkafka::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use crate::server::EventPublisher;

pub struct KafkaPublisher {
    inner: FutureProducer,
}

impl KafkaPublisher {
    pub fn new(bootstrap_server: &str, client_id: &str) -> anyhow::Result<Self> {
        let inner = ClientConfig::new()
            .set("bootstrap.servers", bootstrap_server)
            .set("client.id", client_id)
            .set("acks", "all")
            .set("enable.idempotence", "true")
            .set("max.in.flight.requests.per.connection", "1")
            .set("compression.type", "lz4")
            .create()?;
        Ok(Self { inner })
    }
}

#[async_trait::async_trait]
impl EventPublisher for KafkaPublisher {
    async fn publish(&self, topic: &str, key: &[u8], payload: &[u8]) -> anyhow::Result<()> {
        let rec = FutureRecord::to(topic).key(key).payload(payload);
        self.inner
            .send(rec, Duration::from_secs(10))
            .await
            .map(|_delivery_report| ())
            .map_err(|(e, _msg)| anyhow::anyhow!(e))
    }
}