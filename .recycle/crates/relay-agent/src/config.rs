use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub relay: RelayConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelayConfig {
    #[serde(default = "default_marionette_url")]
    pub marionette_url: String,
    #[serde(default = "default_heartbeat_secs")]
    pub heartbeat_interval_secs: u64,
    pub token: Option<String>,
}

fn default_marionette_url() -> String {
    "ws://localhost:3001/relay".into()
}

fn default_heartbeat_secs() -> u64 {
    5
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let marionette_url = std::env::var("MARIONETTE_URL")
            .unwrap_or_else(|_| "ws://localhost:3001/relay".into());

        Ok(Config {
            relay: RelayConfig {
                marionette_url,
                heartbeat_interval_secs: std::env::var("HEARTBEAT_INTERVAL_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5),
                token: std::env::var("RELAY_TOKEN").ok(),
            },
        })
    }
}
