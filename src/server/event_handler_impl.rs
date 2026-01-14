use crate::domain_model::*;
use crate::server::{EventHandler, HandleOutcome, OutboundQueue};
use std::sync::Arc;

pub struct ConnFanoutHandler {
    outbound_queue: Arc<dyn OutboundQueue>,
}

impl ConnFanoutHandler {
    pub fn new(outbound_queue: Arc<dyn OutboundQueue>) -> Self {
        Self { outbound_queue }
    }
}

#[async_trait::async_trait]
impl EventHandler for ConnFanoutHandler {
    async fn handle(&self, payload: &[u8]) -> anyhow::Result<HandleOutcome> {
        let s2c_envelope_json_value = serde_json::from_slice::<serde_json::Value>(payload)?;
        let s2c_envelope = serde_json::from_value::<S2CEnvelope>(s2c_envelope_json_value)?;

        for r in s2c_envelope.receivers {
            if let Err(e) = self.outbound_queue.enqueue(r, &s2c_envelope.body).await {
                tracing::warn!("outbound queue dropped (offline?): {e}");
            }
        }

        Ok(HandleOutcome::Commit)
    }
}
