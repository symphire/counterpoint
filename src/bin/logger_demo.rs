use counterpoint::logger::*;

fn main() -> anyhow::Result<()> {
    let logger = Logger::new_bootstrap();
    trace!("bootstrap trace log");
    debug!("bootstrap debug log");
    info!("bootstrap info log");

    let config = LogConfig{ filter: "debug".to_string() };
    logger.reload_from_config(&config)?;
    trace!("application trace log");
    debug!("application debug log");
    info!("application info log");

    Ok(())
}