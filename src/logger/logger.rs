use anyhow::{Result, anyhow};
use tracing_subscriber::{
    EnvFilter, Registry, fmt, layer::SubscriberExt, reload, util::SubscriberInitExt,
};

pub struct LogConfig {
    pub filter: String,
}

pub struct Logger {
    reload_handle: reload::Handle<EnvFilter, Registry>,
}

impl Logger {
    pub fn new_bootstrap() -> Self {
        let filter = EnvFilter::new("info");
        let (filter, reload_handle) = reload::Layer::new(filter);

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .init();

        Self { reload_handle }
    }

    pub fn reload_from_config(&self, config: &LogConfig) -> Result<()> {
        let filter = EnvFilter::try_new(&config.filter).map_err(|e| anyhow!(e))?;
        self.reload_handle.reload(filter).map_err(|e| anyhow!(e))?;
        Ok(())
    }
}
