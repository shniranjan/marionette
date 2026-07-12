use crate::config::Config;
use crate::handlers;
use futures_util::{SinkExt, StreamExt};
use relay_protocol::Message;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMsg;

/// Reconnect loop: retry on failure with a 5-second backoff.
pub async fn connect_loop(cfg: Config) -> anyhow::Result<()> {
    loop {
        match connect_and_serve(&cfg).await {
            Ok(()) => {
                tracing::info!("connection closed cleanly, reconnecting in 5s...");
            }
            Err(e) => {
                tracing::warn!(error = %e, "connection error, retrying in 5s...");
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

/// Establish a WebSocket connection to Marionette and process messages.
async fn connect_and_serve(cfg: &Config) -> anyhow::Result<()> {
    tracing::info!(url = %cfg.marionette_url, "connecting to marionette");

    let (ws_stream, _response) = tokio_tungstenite::connect_async(&cfg.marionette_url).await?;
    tracing::info!("Connected to {}", cfg.marionette_url);

    let (mut write, mut read) = ws_stream.split();

    // Channel for streaming events from handlers to the WebSocket.
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Message>();

    loop {
        tokio::select! {
            // Incoming WebSocket messages
            msg = read.next() => {
                match msg {
                    Some(Ok(WsMsg::Text(text))) => {
                        tracing::debug!(len = text.len(), "received text message");

                        match serde_json::from_str::<Message>(&text) {
                            Ok(msg) => {
                                tracing::info!(
                                    id = %msg.id,
                                    msg_type = ?msg.msg_type,
                                    subtype = %msg.subtype,
                                    "received message"
                                );

                                match handlers::dispatch(&msg, &event_tx).await {
                                    Ok(response) => {
                                        let json = serde_json::to_string(&response)?;
                                        write.send(WsMsg::Text(json.into())).await?;
                                    }
                                    Err(err) => {
                                        let error_resp = Message::new_error(
                                            msg.id,
                                            &err.error_code.to_str(),
                                            err.message,
                                            err.details,
                                        );
                                        let json = serde_json::to_string(&error_resp)?;
                                        write.send(WsMsg::Text(json.into())).await?;
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to parse message as relay_protocol::Message");
                            }
                        }
                    }
                    Some(Ok(WsMsg::Close(frame))) => {
                        tracing::info!(?frame, "server closed connection");
                        return Ok(());
                    }
                    Some(Ok(WsMsg::Ping(data))) => {
                        write.send(WsMsg::Pong(data)).await?;
                    }
                    Some(Ok(other)) => {
                        tracing::debug!(?other, "received non-text WebSocket message");
                    }
                    Some(Err(e)) => {
                        return Err(e.into());
                    }
                    None => {
                        tracing::info!("WebSocket stream ended");
                        return Ok(());
                    }
                }
            }

            // Outbound streaming events from handlers
            event = event_rx.recv() => {
                match event {
                    Some(event_msg) => {
                        let json = serde_json::to_string(&event_msg)?;
                        write.send(WsMsg::Text(json.into())).await?;
                    }
                    None => {
                        tracing::debug!("event channel closed");
                    }
                }
            }
        }
    }
}
