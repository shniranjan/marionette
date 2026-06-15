use bollard::Docker;
use std::collections::HashMap;
use tokio::time::{timeout, Duration};

use crate::models::{DockerEndpoint, EndpointStatus};

/// Create a bollard Docker client for a given connection string.
/// Supports unix:// sockets and tcp:// connections.
pub fn create_client(connection: &str) -> Result<Docker, String> {
    if connection.starts_with("unix://") {
        let path = connection.strip_prefix("unix://").unwrap_or("/var/run/docker.sock");
        Docker::connect_with_socket(path, 120, bollard::API_DEFAULT_VERSION)
            .map_err(|e| format!("Failed to connect to Docker socket {}: {}", path, e))
    } else if connection.starts_with("tcp://") {
        Docker::connect_with_http(connection, 120, bollard::API_DEFAULT_VERSION)
            .map_err(|e| format!("Failed to connect to Docker at {}: {}", connection, e))
    } else {
        Err(format!("Unsupported connection scheme: {}", connection))
    }
}

/// Build the initial endpoint map with a local Docker socket.
pub async fn build_initial_endpoints() -> (
    HashMap<String, DockerEndpoint>,
    HashMap<String, Docker>,
    String,
) {
    let local_id = uuid::Uuid::new_v4().to_string();
    let local_connection = std::env::var("DOCKER_HOST")
        .unwrap_or_else(|_| "unix:///var/run/docker.sock".to_string());

    let mut endpoints = HashMap::new();
    let mut clients = HashMap::new();

    let endpoint = DockerEndpoint {
        id: local_id.clone(),
        name: "local".to_string(),
        connection: local_connection.clone(),
        status: EndpointStatus::Disconnected,
        tags: vec!["default".to_string()],
    };

    match create_client(&local_connection) {
        Ok(docker) => {
            clients.insert(local_id.clone(), docker);
            endpoints.insert(
                local_id.clone(),
                DockerEndpoint {
                    status: EndpointStatus::Connected,
                    ..endpoint
                },
            );
        }
        Err(e) => {
            endpoints.insert(
                local_id.clone(),
                DockerEndpoint {
                    status: EndpointStatus::Error(e),
                    ..endpoint
                },
            );
        }
    }

    (endpoints, clients, local_id)
}

/// Get a Docker client for an endpoint, with a 5-second timeout ping check.
pub async fn get_client(
    endpoint_id: &str,
    clients: &HashMap<String, Docker>,
) -> Result<Docker, String> {
    let docker = clients
        .get(endpoint_id)
        .ok_or_else(|| format!("Endpoint not found: {}", endpoint_id))?;

    match timeout(Duration::from_secs(5), docker.ping()).await {
        Ok(Ok(_)) => Ok(docker.clone()),
        Ok(Err(e)) => Err(format!("Docker unreachable: {}", e)),
        Err(_) => Err("Docker connection timed out (5s)".to_string()),
    }
}

/// Classify a volume driver and return (category, migration_advice).
pub fn classify_driver(driver: &str) -> (&'static str, &'static str) {
    match driver {
        "local" | "local-persist" => ("filesystem", "transfer"),
        "nfs" | "cifs" | "smb" => ("network", "reconnect"),
        "rclone" => ("cloud", "reconnect"),
        "rexray" | "cloudstor" => ("cloud_block", "reconnect"),
        "glusterfs" => ("distributed", "reconnect"),
        _ => ("unknown", "warn"),
    }
}

/// Sanitize volume options — mask secrets.
pub fn sanitize_options(
    driver: &str,
    options: &HashMap<String, String>,
) -> HashMap<String, String> {
    let secret_keys: &[&str] = match driver {
        "cifs" | "smb" => &["password", "secret", "credentials"],
        "rclone" => &[
            "s3-access-key-id",
            "s3-secret-access-key",
            "s3-session-token",
            "gcs-service-account-file",
            "azure-account-key",
        ],
        "nfs" => &[],
        _ => &["password", "secret", "key", "token", "credentials", "access-key"],
    };

    options
        .iter()
        .map(|(k, v)| {
            if secret_keys
                .iter()
                .any(|sk| k.to_lowercase().contains(sk))
            {
                (k.clone(), "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}".to_string())
            } else {
                (k.clone(), v.clone())
            }
        })
        .collect()
}

/// Format bytes as human-readable string.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{}B", bytes)
    } else {
        format!("{:.1}{}", size, UNITS[unit_idx])
    }
}
