use tracing_subscriber::fmt::format::FmtSpan;

mod auth;
mod config;
mod handlers;
mod signed;
mod ws;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    tracing_subscriber::fmt()
        .json()
        .with_span_events(FmtSpan::CLOSE)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse()?),
        )
        .init();

    // Initialize Docker client eagerly
    handlers::docker_client();

    let cfg = config::Config::load()?;
    tracing::info!(marionette_url = %cfg.relay.marionette_url, "relay-agent starting");

    ws::connect_loop(cfg).await
}
