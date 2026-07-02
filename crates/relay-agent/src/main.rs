use tracing_subscriber::fmt::format::FmtSpan;

mod auth;
mod config;
mod handlers;
mod signed;
mod ws;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    // Spawn local test server on port 9120 for direct handler testing
    tokio::spawn(local_test_server());

    ws::connect_loop(cfg).await
}

/// Local WebSocket server for direct handler testing.
/// Binds to 0.0.0.0:9120 — accepts raw protocol messages.
async fn local_test_server() {
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    let listener = match TcpListener::bind("0.0.0.0:9120").await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("local test server failed to bind: {}", e);
            return;
        }
    };
    tracing::info!("local test server listening on 0.0.0.0:9120");

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        tokio::spawn(async move {
            let mut ws = match accept_async(stream).await {
                Ok(w) => w,
                Err(_) => return,
            };

            while let Some(Ok(msg)) = ws.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                    match serde_json::from_str::<relay_protocol::Message>(&text) {
                        Ok(req) => {
                            let resp = handlers::dispatch(req).await;
                            if let Some(r) = resp {
                                let json = serde_json::to_string(&r).unwrap_or_default();
                                let _ = ws.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("invalid local test message: {}", e);
                        }
                    }
                }
            }
        });
    }
}
