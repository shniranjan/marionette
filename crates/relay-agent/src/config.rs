/// Relay agent configuration, loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// WebSocket URL of the Marionette controller.
    pub marionette_url: String,
    /// Optional authentication token.
    #[allow(dead_code)]
    pub relay_token: Option<String>,
    /// Docker daemon connection URI.
    pub docker_host: String,
    /// Log level filter (RUST_LOG-compatible).
    #[allow(dead_code)]
    pub log_level: String,
}

impl Config {
    /// Load configuration from environment variables.
    pub fn load() -> Self {
        Self {
            marionette_url: std::env::var("MARIONETTE_URL")
                .unwrap_or_else(|_| "ws://localhost:9119/relay".into()),
            relay_token: std::env::var("RELAY_TOKEN").ok(),
            docker_host: std::env::var("DOCKER_HOST")
                .unwrap_or_else(|_| "unix:///var/run/docker.sock".into()),
            log_level: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        }
    }
}
