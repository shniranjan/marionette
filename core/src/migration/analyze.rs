//! Analyze phase — understand source stack + target readiness.
//!
//! `analyze_migration()` inspects the source container, resolves its
//! compose configuration, discovers volumes, checks target host
//! compatibility, detects database connections, and builds a compose
//! diff — all via relay commands. The result is a compiled
//! `AnalyzeResult` that drives the rest of the pipeline.

use relay_protocol::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

// ── Constants ────────────────────────────────────────────────────────

/// Default timeout for relay commands (seconds).
const RELAY_TIMEOUT: u64 = 30;

/// Comma-separated compose label keys to try when discovering the
/// project working directory.
const COMPOSE_LABELS: &[&str] = &[
    "com.docker.compose.project.working_dir",
    "com.docker.compose.project.config_files",
];

// ── Public types ─────────────────────────────────────────────────────

/// Compiled result of the analyze phase — drives all downstream phases.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResult {
    pub source: HostSummary,
    pub target: HostSummary,
    pub services: Vec<ServicePlan>,
    pub volumes: Vec<VolumePlan>,
    pub networks: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub cross_arch: bool,
    pub cross_arch_services: Vec<String>,
    pub bind_mounts: Vec<BindMount>,
    pub db_connections: Vec<DbConnection>,
    pub compose_file_content: String,
    pub compose_file_path: String,
    pub compose_diff: ComposeDiff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSummary {
    pub hostname: String,
    pub architecture: String,
    pub docker_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicePlan {
    pub name: String,
    pub image: String,
    pub source_arch: String,
    pub target_arch_ok: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env_vars: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_diff: Vec<EnvChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumePlan {
    pub name: String,
    pub driver: String,
    pub size_bytes: u64,
    pub transfer_strategy: String,
    pub database_detected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindMount {
    pub source_path: String,
    pub destination: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbConnection {
    pub container: String,
    #[serde(rename = "type")]
    pub db_type: String, // "postgresql" | "mysql" | "mongodb" | "redis" | "unknown"
    pub host_ref: String, // masked in logs
    pub suggested_target_host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvChange {
    pub key: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComposeDiff {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_changes: Vec<EnvChange>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub volume_renames: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_services: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_services: Vec<String>,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Build a relay request message with a fresh UUID.
fn relay_request(subtype: &str, payload: Value) -> Message {
    Message::new_request(Uuid::new_v4().to_string(), subtype, payload)
}

/// Send a relay command and parse the payload as the expected type T.
async fn relay<T: serde::de::DeserializeOwned>(
    host: &str,
    subtype: &str,
    payload: Value,
    timeout: u64,
) -> Result<T, String> {
    let msg = relay_request(subtype, payload);
    let resp = crate::ws_relay::send_relay_command(host, msg, timeout).await?;
    serde_json::from_value::<T>(resp.payload)
        .map_err(|e| format!("Parse error for '{}': {}", subtype, e))
}

/// Mask credential values for logging — never print secrets.
fn mask_value(val: &str) -> String {
    if val.len() <= 4 {
        "****".to_string()
    } else {
        format!("{}****", &val[..4])
    }
}

/// Classify database type from an env-var key + value heuristic.
fn classify_db_type(env_key: &str, _env_value: &str) -> &'static str {
    let upper = env_key.to_uppercase();
    if upper.contains("POSTGRES") || upper.contains("PG") {
        "postgresql"
    } else if upper.contains("MYSQL") || upper.contains("MARIADB") {
        "mysql"
    } else if upper.contains("MONGO") {
        "mongodb"
    } else if upper.contains("REDIS") {
        "redis"
    } else {
        "unknown"
    }
}

/// Pull a string field from a JSON object (case-insensitive first
/// letter). Returns None when missing or not a string.
fn json_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    // Try exact match first, then lowercased first letter.
    v.get(key)
        .and_then(|x| x.as_str())
        .or_else(|| {
            let mut chars = key.chars();
            let lc_first: String = chars
                .next()
                .map(|c| c.to_lowercase().collect::<String>())
                .unwrap_or_default()
                + chars.as_str();
            v.get(&lc_first).and_then(|x| x.as_str())
        })
}

// ── Volume discovery ─────────────────────────────────────────────────

/// Discover volumes from a container's Mounts array.
async fn discover_volumes(
    host: &str,
    container_inspect: &Value,
) -> Result<(Vec<VolumePlan>, Vec<BindMount>), String> {
    let mut volumes: Vec<VolumePlan> = Vec::new();
    let mut bind_mounts: Vec<BindMount> = Vec::new();

    let mounts = match container_inspect.get("Mounts").and_then(|m| m.as_array()) {
        Some(arr) => arr,
        None => return Ok((volumes, bind_mounts)),
    };

    for mount in mounts {
        let mount_type = mount
            .get("Type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match mount_type {
            "volume" => {
                let name = mount
                    .get("Name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let driver = mount
                    .get("Driver")
                    .and_then(|v| v.as_str())
                    .unwrap_or("local")
                    .to_string();

                // Inspect the volume for driver + mountpoint details.
                let vol_info: Value = relay(
                    host,
                    "docker.inspect",
                    serde_json::json!({"id": &name}),
                    15,
                )
                .await
                .unwrap_or_default();

                // DockerInspectResponse wraps in "inspect_json" field.
                let vol_detail = vol_info
                    .get("inspect_json")
                    .unwrap_or(&vol_info);

                let driver = vol_detail
                    .get("Driver")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&driver)
                    .to_string();

                // Detect database volume by name heuristic.
                let name_lower = name.to_lowercase();
                let (database_detected, db_type) =
                    if name_lower.contains("pg") || name_lower.contains("postgres") {
                        (true, Some("postgresql".to_string()))
                    } else if name_lower.contains("mysql") || name_lower.contains("mariadb") {
                        (true, Some("mysql".to_string()))
                    } else if name_lower.contains("mongo") {
                        (true, Some("mongodb".to_string()))
                    } else if name_lower.contains("redis") {
                        (true, Some("redis".to_string()))
                    } else {
                        (false, None)
                    };

                volumes.push(VolumePlan {
                    name,
                    driver,
                    size_bytes: 0, // estimated later in prepare/transfer
                    transfer_strategy: "lan_pipe".to_string(),
                    database_detected,
                    db_type,
                });
            }
            "bind" => {
                let source_path = mount
                    .get("Source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let destination = mount
                    .get("Destination")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                bind_mounts.push(BindMount {
                    source_path,
                    destination,
                    size_bytes: 0,
                });
            }
            _ => {}
        }
    }

    Ok((volumes, bind_mounts))
}

// ── DB detection ─────────────────────────────────────────────────────

const KNOWN_DB_ENV_VARS: &[&str] = &[
    "POSTGRES_PASSWORD",
    "MYSQL_ROOT_PASSWORD",
    "MONGO_URI",
    "DATABASE_URL",
    "REDIS_PASSWORD",
    "DB_PASSWORD",
    "DB_HOST",
    "DB_PORT",
    "PGHOST",
];

const CREDENTIAL_PATTERNS: &[&str] = &[
    "_PASSWORD",
    "_SECRET",
    "_KEY",
    "_TOKEN",
    "API_KEY",
    "SECRET_KEY",
    "AUTH_TOKEN",
];

/// Walk every service's env vars and return detected DB connections.
fn detect_db_connections(
    services: &[ServicePlan],
    target_host: &str,
) -> Vec<DbConnection> {
    let mut conns: Vec<DbConnection> = Vec::new();

    for svc in services {
        for (env_key, env_value) in &svc.env_vars {
            let upper = env_key.to_uppercase();
            if KNOWN_DB_ENV_VARS
                .iter()
                .any(|known| upper == *known)
            {
                let db_type = classify_db_type(env_key, env_value);
                let suggested = env_value.replace("localhost", target_host);

                // NEVER log the real value.
                tracing::debug!(
                    container = %svc.name,
                    env_key = %env_key,
                    value_masked = %mask_value(env_value),
                    "detected DB connection"
                );

                conns.push(DbConnection {
                    container: svc.name.clone(),
                    db_type: db_type.to_string(),
                    host_ref: mask_value(env_value),
                    suggested_target_host: suggested,
                });
            }
        }
    }

    conns
}

/// Check every service env var for credential patterns. Emits warnings
/// but NEVER logs the values.
fn detect_credentials(services: &[ServicePlan]) -> Vec<String> {
    let mut warnings: Vec<String> = Vec::new();

    for svc in services {
        for env_key in svc.env_vars.keys() {
            let upper = env_key.to_uppercase();
            for pattern in CREDENTIAL_PATTERNS {
                if upper.contains(pattern) {
                    warnings.push(format!(
                        "{}: env var '{}' appears to be a credential — ensure it is set on the target host",
                        svc.name, env_key
                    ));
                    break;
                }
            }
        }
    }

    warnings
}

// ── Compose diff ─────────────────────────────────────────────────────

/// Compare source env vars against suggested target overrides and
/// build a `ComposeDiff`.
fn build_compose_diff(
    services: &[ServicePlan],
    target_host: &str,
) -> ComposeDiff {
    let mut diff = ComposeDiff::default();

    for svc in services {
        for (key, value) in &svc.env_vars {
            let upper = key.to_uppercase();

            // If DB_HOST (or similar) points to localhost, suggest replacing.
            if (upper == "DB_HOST" || upper.contains("_HOST"))
                && (value.contains("localhost") || value == "127.0.0.1")
            {
                diff.env_changes.push(EnvChange {
                    key: key.clone(),
                    old: value.clone(),
                    new: target_host.to_string(),
                });
            }
        }
    }

    diff
}

// ── Main analyze function ────────────────────────────────────────────

/// Understand source stack + target readiness.
///
/// Returns a compiled `AnalyzeResult` that drives the prepare, transfer,
/// and switchover phases. All remote inspection is done via relay
/// commands — nothing calls bollard directly.
pub async fn analyze_migration(
    source_host: &str,
    target_host: &str,
    stack_name: &str,
) -> Result<AnalyzeResult, String> {
    let mut warnings: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // ── 1. Resolve source container ──────────────────────────────────
    tracing::info!(%source_host, %stack_name, "analyze: inspecting source container");

    let inspect_resp: Value = relay(
        source_host,
        "docker.inspect",
        serde_json::json!({"container": stack_name}),
        RELAY_TIMEOUT,
    )
    .await
    .map_err(|e| format!("Source container inspect failed: {}", e))?;

    let inspect_json = inspect_resp
        .get("inspect_json")
        .cloned()
        .unwrap_or_else(|| inspect_resp.clone());

    let config = inspect_json
        .get("Config")
        .ok_or("Container inspect missing 'Config'")?;

    let env_list: Vec<String> = config
        .get("Env")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let image = config
        .get("Image")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let labels = config.get("Labels").cloned().unwrap_or(Value::Null);
    let source_arch = inspect_json
        .get("Architecture")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // ── 2. Fetch resolved compose config from source ─────────────────
    let working_dir = COMPOSE_LABELS
        .iter()
        .find_map(|label| labels.get(label)?.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("/opt/stacks/{}", stack_name));

    tracing::info!(%working_dir, "analyze: fetching compose config");

    let compose_config: Value = relay(
        source_host,
        "compose.config",
        serde_json::json!({
            "project_dir": &working_dir,
            "project_name": stack_name,
            "file": null,
        }),
        RELAY_TIMEOUT,
    )
    .await
    .unwrap_or_else(|e| {
        warnings.push(format!("compose.config failed: {}", e));
        Value::Null
    });

    let config_yaml = compose_config
        .get("config_yaml")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Parse services from resolved compose YAML.
    let mut services: Vec<ServicePlan> = Vec::new();
    if !config_yaml.is_empty() {
        let parsed: Value = serde_yaml::from_str(config_yaml).unwrap_or_default();
        if let Some(svc_map) = parsed.get("services").and_then(|s| s.as_object()) {
            for (svc_name, svc_def) in svc_map {
                let svc_image = svc_def
                    .get("image")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let svc_ports: Vec<String> = svc_def
                    .get("ports")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|p| p.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let svc_env: HashMap<String, String> = svc_def
                    .get("environment")
                    .and_then(|v| {
                        // environment can be a map or an array of "KEY=VALUE"
                        if let Some(obj) = v.as_object() {
                            Some(
                                obj.iter()
                                    .map(|(k, val)| {
                                        (k.clone(), val.as_str().unwrap_or("").to_string())
                                    })
                                    .collect(),
                            )
                        } else if let Some(arr) = v.as_array() {
                            let mut m = HashMap::new();
                            for item in arr {
                                if let Some(s) = item.as_str() {
                                    if let Some((k, val)) = s.split_once('=') {
                                        m.insert(k.to_string(), val.to_string());
                                    }
                                }
                            }
                            Some(m)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                services.push(ServicePlan {
                    name: svc_name.clone(),
                    image: svc_image,
                    source_arch: source_arch.clone(),
                    target_arch_ok: true, // checked in step 6
                    ports: svc_ports,
                    env_vars: svc_env,
                    env_diff: Vec::new(),
                });
            }
        }
    }

    // If compose config didn't yield services, parse them from the
    // container's env vars (fallback heuristic).
    if services.is_empty() {
        let svc_name = stack_name.to_string();
        let svc_env: HashMap<String, String> = env_list
            .iter()
            .filter_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                Some((k.to_string(), v.to_string()))
            })
            .collect();
        services.push(ServicePlan {
            name: svc_name,
            image,
            source_arch: source_arch.clone(),
            target_arch_ok: true,
            ports: Vec::new(),
            env_vars: svc_env,
            env_diff: Vec::new(),
        });
    }

    // ── 3. Fetch raw compose file from source ────────────────────────
    let compose_filename = labels
        .get("com.docker.compose.project.config_files")
        .and_then(|v| v.as_str())
        .map(|s| {
            // Extract last path segment from the config_files label
            // which is typically a full path or comma-separated path.
            s.split(',').next().unwrap_or("docker-compose.yml")
        })
        .unwrap_or("docker-compose.yml");

    let compose_path = format!("{}/{}", working_dir.trim_end_matches('/'), compose_filename.trim_start_matches('/'));

    tracing::info!(%compose_path, "analyze: fetching raw compose file");

    let fs_read: Value = relay(
        source_host,
        "fs.read",
        serde_json::json!({"path": &compose_path, "encoding": "utf8"}),
        RELAY_TIMEOUT,
    )
    .await
    .unwrap_or_else(|e| {
        warnings.push(format!("fs.read compose file failed: {}", e));
        Value::Null
    });

    let compose_file_content = fs_read
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // ── 4. Discover volumes ──────────────────────────────────────────
    let (mut volumes, bind_mounts) = discover_volumes(source_host, &inspect_json).await?;

    // ── 5. Get target host info ──────────────────────────────────────
    tracing::info!(%target_host, "analyze: fetching target host info");

    let target_info: Value = relay(
        target_host,
        "host.info",
        serde_json::json!({}),
        RELAY_TIMEOUT,
    )
    .await
    .map_err(|e| format!("Target host.info failed: {}", e))?;

    let target_docker = target_info
        .get("docker")
        .unwrap_or(&Value::Null);
    let target_arch = json_str(target_docker, "architecture")
        .unwrap_or("unknown")
        .to_string();
    let target_docker_version = json_str(target_docker, "version")
        .unwrap_or("unknown")
        .to_string();
    let target_compose_version = json_str(target_docker, "compose_version")
        .map(String::from);

    // ── 6. Check architecture compatibility ──────────────────────────
    let cross_arch = source_arch != target_arch;
    let mut cross_arch_services: Vec<String> = Vec::new();

    if cross_arch {
        warnings.push(format!(
            "CROSS_ARCH: source is {} but target is {} — multi-arch images required",
            source_arch, target_arch
        ));
    }

    // For each service, check image availability on target.
    let mut seen_images = std::collections::HashSet::new();
    for svc in &mut services {
        if svc.image.is_empty() || svc.image == "unknown" {
            continue;
        }
        if !seen_images.insert(svc.image.clone()) {
            // Already checked — copy result.
            continue;
        }

        let img_check: Result<Value, String> = relay(
            target_host,
            "image.ensure",
            serde_json::json!({"image": &svc.image}),
            RELAY_TIMEOUT,
        )
        .await;

        match img_check {
            Ok(resp) => {
                let pulled = resp
                    .get("pulled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // pulled == false means image already present.
                let image_ok = pulled || !cross_arch;
                if cross_arch && !image_ok {
                    cross_arch_services.push(svc.name.clone());
                    warnings.push(format!(
                        "CROSS_ARCH: service '{}' image '{}' may not be available for {}",
                        svc.name, svc.image, target_arch
                    ));
                }
                svc.target_arch_ok = image_ok;
            }
            Err(e) => {
                warnings.push(format!(
                    "image.ensure failed for '{}': {}",
                    svc.image, e
                ));
                svc.target_arch_ok = !cross_arch;
                if cross_arch {
                    cross_arch_services.push(svc.name.clone());
                }
            }
        }
    }

    // ── 7. Detect database connections ───────────────────────────────
    let db_connections = detect_db_connections(&services, target_host);

    // ── 8. Detect credentials ────────────────────────────────────────
    let cred_warnings = detect_credentials(&services);
    warnings.extend(cred_warnings);

    // ── 9. Build compose diff ────────────────────────────────────────
    let compose_diff = build_compose_diff(&services, target_host);

    // Merge env_diff into each service.
    for svc in &mut services {
        svc.env_diff = compose_diff
            .env_changes
            .iter()
            .filter(|ch| svc.env_vars.contains_key(&ch.key))
            .cloned()
            .collect();
    }

    // ── 10. Compile and return ───────────────────────────────────────
    let source_docker_version = inspect_json
        .get("DockerVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    tracing::info!(
        source = %source_host,
        target = %target_host,
        services = services.len(),
        volumes = volumes.len(),
        warnings = warnings.len(),
        errors = errors.len(),
        cross_arch = cross_arch,
        "analyze: complete"
    );

    Ok(AnalyzeResult {
        source: HostSummary {
            hostname: source_host.to_string(),
            architecture: source_arch,
            docker_version: source_docker_version,
            compose_version: target_compose_version.clone(), // same compose on both
        },
        target: HostSummary {
            hostname: target_host.to_string(),
            architecture: target_arch,
            docker_version: target_docker_version,
            compose_version: target_compose_version,
        },
        services,
        volumes,
        networks: Vec::new(),
        warnings,
        errors,
        cross_arch,
        cross_arch_services,
        bind_mounts,
        db_connections,
        compose_file_content,
        compose_file_path: compose_path,
        compose_diff,
    })
}
