use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use bollard::container::{
    CreateContainerOptions, ListContainersOptions, RemoveContainerOptions,
    StartContainerOptions,
};
use std::sync::Arc;

use crate::docker::*;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── Volume Deep Inspection ────────────────────────────────────

pub async fn deep_inspect_volume(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<VolumeDeepInspection> {
    let endpoint_id = params
        .endpoint
        .unwrap_or_else(|| state.default_endpoint.clone());

    // Get client for volume inspection
    let clients = state.clients.read().await;
    let docker = get_client(&endpoint_id, &clients)
        .await
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    drop(clients);

    // Inspect the volume
    let vol_info = docker
        .inspect_volume(&name)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))?;

    let driver = vol_info.driver.clone();
    let (category, migration_advice) = classify_driver(&driver);
    // Bollard 0.17: options/labels are HashMap not Option
    let options = vol_info.options.clone();
    let options_sanitized = sanitize_options(&driver, &options);
    let labels = vol_info.labels.clone();
    let mountpoint = vol_info.mountpoint.clone();

    // Find containers using this volume
    let clients = state.clients.read().await;
    let docker = get_client(&endpoint_id, &clients)
        .await
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    drop(clients);

    let containers = docker
        .list_containers::<String>(Some(ListContainersOptions {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let mut used_by: Vec<String> = Vec::new();
    for c in &containers {
        if let Some(mounts) = c.mounts.as_ref() {
            for m in mounts {
                if m.name.as_deref() == Some(&name) {
                    let container_name = c
                        .names
                        .as_ref()
                        .and_then(|n| n.first())
                        .map(|n| n.trim_start_matches('/').to_string())
                        .unwrap_or_else(|| c.id.clone().unwrap_or_default());
                    used_by.push(container_name);
                    break;
                }
            }
        }
    }

    let mount_count = used_by.len();
    let shared = mount_count > 1;
    let needs_chown = category == "filesystem";

    // Compute size by mounting a temp alpine container and running du
    let (size_bytes, size_human, file_count, last_modified) =
        compute_volume_size(&docker, &name).await;

    let inspection = VolumeDeepInspection {
        name: vol_info.name.clone(),
        driver: driver.clone(),
        driver_type: driver.clone(),
        driver_category: category.to_string(),
        migration_advice: migration_advice.to_string(),
        mountpoint,
        size_bytes,
        size_human,
        file_count,
        last_modified,
        used_by,
        shared,
        options,
        options_sanitized,
        labels,
        scope: format!("{:?}", vol_info.scope).to_lowercase(),
        needs_chown,
        mount_count,
    };

    Ok(Json(inspection))
}

/// Run a temporary alpine container with the volume mounted at /data,
/// then execute du/find/stat via docker exec. Returns size metrics or None on failure.
async fn compute_volume_size(
    docker: &bollard::Docker,
    volume_name: &str,
) -> (Option<u64>, Option<String>, Option<u64>, Option<String>) {
    let container_name = format!("marionette-volsize-{}", uuid::Uuid::new_v4());

    // Create a temporary alpine container with the volume mounted at /data
    let create_result = docker
        .create_container(
            Some(CreateContainerOptions {
                name: container_name.clone(),
                platform: None,
            }),
            bollard::container::Config {
                image: Some("alpine:latest"),
                // Bollard 0.17: cmd is Vec<&str>
                cmd: Some(vec!["sleep", "3600"]),
                host_config: Some(bollard::models::HostConfig {
                    mounts: Some(vec![bollard::models::Mount {
                        target: Some("/data".to_string()),
                        source: Some(volume_name.to_string()),
                        typ: Some(bollard::models::MountTypeEnum::VOLUME),
                        read_only: Some(false),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await;

    let container_id = match create_result {
        Ok(resp) => resp.id,
        Err(_) => return (None, None, None, None),
    };

    // Start the container
    if docker
        .start_container(&container_id, None::<StartContainerOptions<String>>)
        .await
        .is_err()
    {
        cleanup_container(docker, &container_id).await;
        return (None, None, None, None);
    }

    // Give it a moment to initialize
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    // Run du -sb /data via docker exec (using host docker CLI for simplicity)
    let size_bytes = run_docker_exec(&container_id, &["du", "-sb", "/data"])
        .await
        .and_then(|output| {
            output
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        });

    // Count files
    let file_count = run_docker_exec(
        &container_id,
        &["sh", "-c", "find /data -type f 2>/dev/null | wc -l"],
    )
    .await
    .and_then(|output| output.trim().parse::<u64>().ok());

    // Last modification time
    let last_modified = run_docker_exec(
        &container_id,
        &["sh", "-c", "stat -c %Y /data 2>/dev/null || echo ''"],
    )
    .await
    .and_then(|s| {
        let s = s.trim().to_string();
        if s.is_empty() {
            None
        } else {
            s.parse::<i64>().ok().and_then(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.to_rfc3339())
            })
        }
    });

    // Cleanup
    cleanup_container(docker, &container_id).await;

    let size_human = size_bytes.map(human_bytes);

    (size_bytes, size_human, file_count, last_modified)
}

/// Run a command inside a container using `docker exec` on the host.
async fn run_docker_exec(container_id: &str, args: &[&str]) -> Option<String> {
    let mut cmd = vec!["exec", container_id];
    cmd.extend_from_slice(args);

    let output = tokio::process::Command::new("docker")
        .args(&cmd)
        .output()
        .await
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        // Try to parse something from stdout even on partial failure
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.trim().is_empty() {
            None
        } else {
            Some(stdout)
        }
    }
}

async fn cleanup_container(docker: &bollard::Docker, container_id: &str) {
    let _ = docker
        .stop_container(container_id, None::<bollard::container::StopContainerOptions>)
        .await;
    let _ = docker
        .remove_container(
            container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}
