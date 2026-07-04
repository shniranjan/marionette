use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use bollard::container::CreateContainerOptions;
use bollard::models::{HostConfig, PortBinding};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::helpers;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── List Templates ─────────────────────────────────────────────

pub async fn list_templates(
    State(state): State<Arc<crate::AppState>>,
) -> ApiResult<Vec<Template>> {
    let templates = state.registry.db().list_templates();
    Ok(Json(templates))
}

// ── Get Template ───────────────────────────────────────────────

pub async fn get_template(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Template> {
    state
        .registry
        .db()
        .get_template(&id)
        .map(Json)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Template '{}' not found", id)))
}

// ── Create Template ────────────────────────────────────────────

pub async fn create_template(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<TemplateCreateRequest>,
) -> ApiResult<Template> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let template = Template {
        id,
        name: body.name,
        description: body.description,
        image: body.image,
        ports: body.ports,
        env_vars: body.env_vars,
        volumes: body.volumes,
        restart_policy: body.restart_policy,
        labels: body.labels,
        created_at: now,
    };

    state.registry.db().save_template(&template);

    Ok(Json(template))
}

// ── Delete Template ────────────────────────────────────────────

pub async fn delete_template(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    if state.registry.db().get_template(&id).is_none() {
        return Err(error(
            StatusCode::NOT_FOUND,
            &format!("Template '{}' not found", id),
        ));
    }

    state.registry.db().delete_template(&id);

    Ok(Json(serde_json::json!({"status": "deleted", "id": id})))
}

// ── Deploy from Template ───────────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployQuery {
    #[serde(default)]
    pub endpoint: Option<String>,
}

pub async fn deploy_template(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<DeployQuery>,
) -> ApiResult<serde_json::Value> {
    let template = state
        .registry
        .db()
        .get_template(&id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Template '{}' not found", id)))?;

    let docker = helpers::resolve_client(&state, query.endpoint.as_deref()).await?;

    let container_name = format!("{}-{}", template.name, Uuid::new_v4().to_string().split('-').next().unwrap_or("0000"));

    // Parse ports from JSON string to PortBindings
    let ports_str: String = template.ports;
    let port_bindings: Option<HashMap<String, Option<Vec<PortBinding>>>> =
        if ports_str != "[]" {
            serde_json::from_str::<Vec<serde_json::Value>>(&ports_str).ok().map(|arr| {
                let mut bindings = HashMap::new();
                for entry in arr {
                    let container_port = entry.get("containerPort").and_then(|v| v.as_u64()).map(|p| format!("{}/tcp", p));
                    let host_port = entry.get("hostPort").and_then(|v| v.as_u64()).map(|p| p.to_string());
                    if let Some(cp) = container_port {
                        let binding = PortBinding {
                            host_ip: Some("0.0.0.0".to_string()),
                            host_port: host_port,
                        };
                        bindings.insert(cp, Some(vec![binding]));
                    }
                }
                bindings
            })
        } else {
            None
        };

    // Parse env_vars JSON
    let env: Option<Vec<String>> = {
        let env_str: String = template.env_vars;
        if env_str != "{}" {
            serde_json::from_str::<HashMap<String, String>>(&env_str).ok().map(|map| {
                map.into_iter().map(|(k, v)| format!("{}={}", k, v)).collect()
            })
        } else {
            None
        }
    };

    // Parse mounts from volumes JSON
    let mounts: Option<Vec<bollard::models::Mount>> = {
        let vols_str: String = template.volumes;
        if vols_str != "[]" {
            serde_json::from_str::<Vec<serde_json::Value>>(&vols_str).ok().map(|arr| {
                arr.into_iter().map(|v| bollard::models::Mount {
                    target: v.get("destination").and_then(|d| d.as_str()).map(|s| s.to_string()),
                    source: v.get("source").and_then(|s| s.as_str()).map(|s| s.to_string()),
                    typ: Some(bollard::models::MountTypeEnum::BIND),
                    read_only: v.get("mode").and_then(|m| m.as_str()).map(|m| m == "ro"),
                    ..Default::default()
                }).collect()
            })
        } else {
            None
        }
    };

    // Parse labels JSON
    let labels: Option<HashMap<String, String>> = {
        let labels_str: String = template.labels;
        if labels_str != "{}" {
            serde_json::from_str(&labels_str).ok()
        } else {
            None
        }
    };

    let restart_policy = if template.restart_policy != "no" && !template.restart_policy.is_empty() {
        let name = match template.restart_policy.as_str() {
            "always" => bollard::models::RestartPolicyNameEnum::ALWAYS,
            "on-failure" => bollard::models::RestartPolicyNameEnum::ON_FAILURE,
            "unless-stopped" => bollard::models::RestartPolicyNameEnum::UNLESS_STOPPED,
            _ => bollard::models::RestartPolicyNameEnum::NO,
        };
        Some(bollard::models::RestartPolicy {
            name: Some(name),
            maximum_retry_count: None,
        })
    } else {
        None
    };

    let config = bollard::container::Config {
        image: Some(template.image),
        env,
        labels,
        host_config: Some(HostConfig {
            port_bindings,
            mounts,
            restart_policy,
            ..Default::default()
        }),
        ..Default::default()
    };

    let result = docker
        .create_container(
            Some(CreateContainerOptions {
                name: container_name.clone(),
                platform: None,
            }),
            config,
        )
        .await
        .map_err(|e| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to create container: {}", e),
            )
        })?;

    // Start the container
    let _ = docker
        .start_container(&result.id, None::<bollard::container::StartContainerOptions<String>>)
        .await
        .map_err(|e| {
            // Container created but start failed — still report partial success
            tracing::warn!("Container {} created but failed to start: {}", result.id, e);
        });

    Ok(Json(serde_json::json!({
        "status": "deployed",
        "containerId": result.id,
        "containerName": container_name,
        "templateId": template.id,
    })))
}
