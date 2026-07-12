use tracing_subscriber::EnvFilter;

mod auth;
mod config;
mod docker;
mod handlers;
mod ops;
mod signed;
mod transfer;
mod ws;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let version = env!("CARGO_PKG_VERSION");
    tracing::info!("relay-agent v{} starting", version);

    let cfg = config::Config::load();
    tracing::info!(
        marionette_url = %cfg.marionette_url,
        docker_host = %cfg.docker_host,
        "configuration loaded"
    );

    ws::connect_loop(cfg).await
}
