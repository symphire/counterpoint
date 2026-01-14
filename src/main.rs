use counterpoint::api;
use counterpoint::logger::*;
use counterpoint::server::*;
use counterpoint::settings::*;
use std::fs;
use std::sync::Arc;
use tokio::signal;
use warp::Filter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let logger = Logger::new_bootstrap();

    let project_settings = parse_settings(cli.settings.as_deref())?;
    info!(?project_settings);
    let logger_config = LogConfig {
        filter: project_settings.log.filter.clone(),
    };
    logger.reload_from_config(&logger_config)?;

    let address: std::net::SocketAddr = project_settings.http.address.parse()?;
    if !fs::metadata(&project_settings.http.cert_path)?.is_file() {
        return Err(anyhow::anyhow!(
            "TLS cert is not a regular file: {:?}",
            project_settings.http.cert_path
        ));
    }
    if !fs::metadata(&project_settings.http.key_path)?.is_file() {
        return Err(anyhow::anyhow!(
            "TLS key is not a regular file: {:?}",
            project_settings.http.key_path
        ));
    }

    let server = Arc::new(Server::try_new(&project_settings).await?);

    let api_v1 = warp::path("api")
        .and(warp::path("v1"))
        .and(api::v1::routes(server.clone()))
        .recover(api::v1::recover_error);

    warp::serve(api_v1)
        .tls()
        .cert_path(project_settings.http.cert_path.clone())
        .key_path(project_settings.http.key_path.clone())
        .bind_with_graceful_shutdown(address, async {
            signal::ctrl_c().await.expect("Could not register SIGINT");
        })
        .1
        .await;

    let shutdown_timeout = std::time::Duration::from_secs(100);
    match tokio::time::timeout(shutdown_timeout, server.shutdown()).await {
        Ok(_) => tracing::info!("server shutdown successfully"),
        Err(_) => tracing::error!("server shutdown timed out"),
    }

    Ok(())
}
