//! Docker client factory — local Unix socket and remote TCP.
//!
//! Provides simple constructors for bollard::Docker clients. The local
//! client connects to the default Docker socket; remote clients connect
//! via TCP with optional TLS.

use bollard::Docker;

/// Connect to the local Docker daemon via the default Unix socket.
///
/// Returns a `Docker` client connected to `unix:///var/run/docker.sock`
/// with a 120-second timeout.
pub fn local_client() -> Result<Docker, String> {
    Docker::connect_with_unix("unix:///var/run/docker.sock", 120, bollard::API_DEFAULT_VERSION)
        .map_err(|e| format!("Failed to connect to local Docker daemon: {}", e))
}

/// Connect to a remote Docker daemon via TCP.
///
/// `host` can be:
/// - A bare hostname or IP → `tcp://host:2375`
/// - A URL with scheme already present (e.g. `https://host:2376`) → used as-is
/// - A URL with `tcp://` scheme → used as-is
///
/// Returns a `Docker` client connected to the resolved URL with a 120-second timeout.
pub fn remote_client(host: &str) -> Result<Docker, String> {
    let url = if host.starts_with("http://")
        || host.starts_with("https://")
        || host.starts_with("tcp://")
    {
        host.to_string()
    } else {
        format!("tcp://{}:2375", host)
    };
    Docker::connect_with_http(&url, 120, bollard::API_DEFAULT_VERSION)
        .map_err(|e| format!("Failed to connect to remote Docker daemon at {}: {}", url, e))
}
