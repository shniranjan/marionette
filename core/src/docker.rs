use bollard::Docker;
use std::collections::HashMap;

use crate::models::{DockerEndpoint, EndpointStatus};

/// Create a bollard Docker client for a given connection string.
/// Supports unix:// sockets, tcp://, https:// (TLS), and ssh:// connections.
/// `cert_path` overrides DOCKER_CERT_PATH env var for per-endpoint TLS certs.
pub fn create_client(connection: &str, cert_path: Option<&str>) -> Result<Docker, String> {
    if connection.starts_with("unix://") {
        let path = connection.strip_prefix("unix://").unwrap_or("/var/run/docker.sock");
        Docker::connect_with_socket(path, 120, bollard::API_DEFAULT_VERSION)
            .map_err(|e| format!("Failed to connect to Docker socket {}: {}", path, e))
    } else if connection.starts_with("https://") || connection.starts_with("tcp+tls://") {
        // TLS connection — prefers per-endpoint cert_path, falls back to DOCKER_CERT_PATH
        let addr = connection
            .strip_prefix("https://")
            .or_else(|| connection.strip_prefix("tcp+tls://"))
            .unwrap_or(connection);
        use std::path::Path;
        let cert_dir = cert_path
            .map(|p| p.to_string())
            .or_else(|| std::env::var("DOCKER_CERT_PATH").ok())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                format!("{}/.docker", home)
            });
        let ssl_key = Path::new(&cert_dir).join("key.pem");
        let ssl_cert = Path::new(&cert_dir).join("cert.pem");
        let ssl_ca = Path::new(&cert_dir).join("ca.pem");
        Docker::connect_with_ssl(addr, &ssl_key, &ssl_cert, &ssl_ca, 120, bollard::API_DEFAULT_VERSION)
            .map_err(|e| format!("Failed to connect to Docker via TLS at {}: {}. Ensure cert dir has ca.pem, cert.pem, key.pem", addr, e))
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
        cert_path: None,
    };

    match create_client(&local_connection, None) {
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
