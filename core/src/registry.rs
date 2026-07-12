//! Endpoint registry — relay connection state and endpoint info.
//!
//! Part of the controller-bridge per tunnel-loom spec §5.3.
//! Provides the data types (EndpointInfo, RelayState, RelayCommand) shared
//! between the registry and WebSocket relay handler.

use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

/// Public information about a registered endpoint exposed via API.
#[derive(Debug, Clone, Serialize)]
pub struct EndpointInfo {
    pub id: i64,
    pub name: String,
    pub hostname: String,
    pub relay_connected: bool,
    pub relay_hostname: Option<String>,
    pub arch: Option<String>,
    pub os: Option<String>,
    pub docker_version: Option<String>,
}

/// Internal state for a single connected relay.
///
/// Holds the command sender (so the controller can send commands to the
/// relay's connection task) and the endpoint metadata.
pub struct RelayState {
    /// Send commands into the relay connection's main loop.
    pub cmd_tx: mpsc::UnboundedSender<RelayCommand>,
    /// Public endpoint metadata.
    pub info: EndpointInfo,
}

/// A command dispatched to a relay, carrying a oneshot channel for the
/// final response.
pub struct RelayCommand {
    /// The relay-protocol message to forward to the relay agent.
    pub message: relay_protocol::Message,
    /// Channel for the relay to send back the final response.
    pub response_tx: oneshot::Sender<relay_protocol::Message>,
}

/// Registry mapping hostname → relay connection state.
///
/// This is a plain wrapper around a HashMap that provides the canonical
/// register / get / list API. The global singleton lives in `ws_relay::RELAYS`.
pub struct EndpointRegistry {
    relays: HashMap<String, RelayState>,
}

impl EndpointRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            relays: HashMap::new(),
        }
    }

    /// Register a relay under the given hostname.
    pub fn register(&mut self, hostname: String, state: RelayState) {
        self.relays.insert(hostname, state);
    }

    /// Look up a relay by hostname.
    pub fn get(&self, hostname: &str) -> Option<&RelayState> {
        self.relays.get(hostname)
    }

    /// Get a mutable reference to a relay by hostname.
    pub fn get_mut(&mut self, hostname: &str) -> Option<&mut RelayState> {
        self.relays.get_mut(hostname)
    }

    /// Remove a relay by hostname, returning its state if it was present.
    pub fn remove(&mut self, hostname: &str) -> Option<RelayState> {
        self.relays.remove(hostname)
    }

    /// List all registered endpoint info (public metadata only).
    pub fn list(&self) -> Vec<EndpointInfo> {
        self.relays.values().map(|s| s.info.clone()).collect()
    }

    /// Number of registered relays.
    pub fn count(&self) -> usize {
        self.relays.len()
    }
}

impl Default for EndpointRegistry {
    fn default() -> Self {
        Self::new()
    }
}
