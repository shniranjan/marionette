//! Prepare phase — provision the target host for migration.
//!
//! `prepare_migration()` writes the compose file, validates it, pulls all
//! required images, creates volumes, and runs pre-flight checks — all via
//! relay commands. Returns a `PrepareResult` that gates the transfer phase.

use relay_protocol::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::time::Instant;
use uuid::Uuid;

use super::analyze::{AnalyzeResult, ComposeDiff};

// ── Constants ────────────────────────────────────────────────────────

/// Default timeout for relay commands (seconds).
const RELAY_TIMEOUT: u64 = 30;

/// Longer timeout for image pulls.
const IMAGE_PULL_TIMEOUT: u64 = 120;

// ── Public types ─────────────────────────────────────────────────────

/// Outcome for a single image pull during the prepare phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageResult {
    pub image: String,
    /// "pulled", "already_present", or "failed"
    pub action: String,
    pub architecture: String,
}

/// Compiled result of the prepare phase — gates the transfer phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareResult {
    pub success: bool,
    pub compose_file_written: bool,
    pub compose_file_path: String,
    pub compose_valid: bool,
    pub images: Vec<ImageResult>,
    pub volumes_created: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes_existing: Vec<String>,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Build a relay request message with a fresh UUID.
fn relay_request(subtype: &str, payload: Value) -> Message {
    Message::new_request(Uuid::new_v4().to_string(), subtype, payload)
}

/// Send a relay command and return the response message.
async fn relay_cmd(
    host: &str,
    subtype: &str,
    payload: Value,
    timeout: u64,
) -> Result<Message, String> {
    let msg = relay_request(subtype, payload);
    crate::ws_relay::send_relay_command(host, msg, timeout).await
}

// ── Compose edits ────────────────────────────────────────────────────

/// Apply user edits from the compose diff to the raw compose file content.
///
/// Performs string-level substitutions for env var changes and volume
/// renames. This is a best-effort approach — complex diffs may require
/// full YAML parsing (future enhancement).
fn apply_compose_edits(content: &str, diff: &ComposeDiff) -> String {
    let mut result = content.to_string();

    // Apply env var substitutions.
    for change in &diff.env_changes {
        // Try "KEY=old_value" → "KEY=new_value" pattern.
        let old = format!("{}={}", change.key, change.old);
        let new = format!("{}={}", change.key, change.new);
        if result.contains(&old) {
            result = result.replace(&old, &new);
        } else {
            // Fallback: replace just the value in any context.
            result = result.replace(&change.old, &change.new);
        }
    }

    // Apply volume renames.
    for (old_vol, new_vol) in &diff.volume_renames {
        result = result.replace(old_vol, new_vol);
    }

    result
}

// ── Main prepare function ────────────────────────────────────────────

/// Pre-create everything on the target host: compose file, images,
/// volumes, and pre-flight checks.
///
/// All remote operations flow through `crate::ws_relay::send_relay_command()`.
/// Returns a `PrepareResult` with per-operation status and any warnings.
pub async fn prepare_migration(
    target_host: &str,
    plan: &AnalyzeResult,
) -> Result<PrepareResult, String> {
    let start = Instant::now();
    let mut warnings: Vec<String> = Vec::new();

    // ── 1. Write the compose file to target host ─────────────────────

    let final_compose = apply_compose_edits(&plan.compose_file_content, &plan.compose_diff);
    let target_path = plan.compose_file_path.clone();

    tracing::info!(%target_path, size = final_compose.len(), "prepare: writing compose file to target");

    let fs_resp = relay_cmd(
        target_host,
        "fs.write",
        serde_json::json!({
            "path": &target_path,
            "content": &final_compose,
            "encoding": "utf8",
        }),
        RELAY_TIMEOUT,
    )
    .await
    .map_err(|e| format!("fs.write compose file failed: {}", e))?;

    let compose_file_written = fs_resp
        .payload
        .get("bytes_written")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        > 0;

    if !compose_file_written {
        warnings.push("Compose file write reported 0 bytes written".into());
    }

    // ── 2. Validate compose on target ────────────────────────────────

    let project_dir = std::path::Path::new(&target_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    tracing::info!(%project_dir, "prepare: validating compose on target");

    let compose_valid = match relay_cmd(
        target_host,
        "compose.config",
        serde_json::json!({
            "project_dir": &project_dir,
            "file": null,
        }),
        RELAY_TIMEOUT,
    )
    .await
    {
        Ok(resp) => {
            let has_config = resp
                .payload
                .get("config_yaml")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if !has_config {
                warnings.push("Compose validation returned empty config_yaml".into());
            }
            has_config
        }
        Err(e) => {
            warnings.push(format!("Compose validation failed: {}", e));
            false
        }
    };

    // ── 3. Pull all required images (dedup by image name) ────────────

    tracing::info!(service_count = plan.services.len(), "prepare: pulling images");

    let mut images: Vec<ImageResult> = Vec::new();
    let mut seen_images = HashSet::new();

    for svc in &plan.services {
        if svc.image.is_empty() || svc.image == "unknown" {
            continue;
        }
        if !seen_images.insert(svc.image.clone()) {
            continue; // Already pulled or attempted.
        }

        match relay_cmd(
            target_host,
            "image.ensure",
            serde_json::json!({"image": &svc.image}),
            IMAGE_PULL_TIMEOUT,
        )
        .await
        {
            Ok(resp) => {
                let pulled = resp
                    .payload
                    .get("pulled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let image_id = resp
                    .payload
                    .get("image_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let action = if pulled {
                    tracing::info!(%svc.image, %image_id, "prepare: image pulled");
                    "pulled".to_string()
                } else {
                    tracing::info!(%svc.image, %image_id, "prepare: image already present");
                    "already_present".to_string()
                };

                images.push(ImageResult {
                    image: svc.image.clone(),
                    action,
                    architecture: plan.target.architecture.clone(),
                });
            }
            Err(e) => {
                let warn = format!("Image pull failed for '{}': {}", svc.image, e);
                tracing::warn!(%warn);
                warnings.push(warn);
                images.push(ImageResult {
                    image: svc.image.clone(),
                    action: "failed".to_string(),
                    architecture: plan.target.architecture.clone(),
                });
            }
        }
    }

    // ── 4. Create volumes on target ──────────────────────────────────

    tracing::info!(volume_count = plan.volumes.len(), "prepare: ensuring volumes on target");

    let mut volumes_created: Vec<String> = Vec::new();
    let mut volumes_existing: Vec<String> = Vec::new();

    // Find a running container on target to exec docker volume create.
    let exec_container: Option<String> = relay_cmd(
        target_host,
        "docker.ps",
        serde_json::json!({"all": false}),
        RELAY_TIMEOUT,
    )
    .await
    .ok()
    .and_then(|r| {
        r.payload
            .get("containers")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| {
                c.get("name")
                    .and_then(|n| n.as_str())
                    .or_else(|| {
                        c.get("Names")
                            .and_then(|n| n.as_array())
                            .and_then(|a| a.first())
                            .and_then(|v| v.as_str())
                    })
                    .map(String::from)
            })
    });

    for vol in &plan.volumes {
        // Check if volume already exists on target.
        match relay_cmd(
            target_host,
            "docker.inspect",
            serde_json::json!({"id": &vol.name}),
            RELAY_TIMEOUT,
        )
        .await
        {
            Ok(_) => {
                tracing::info!(volume = %vol.name, "prepare: volume already exists on target");
                volumes_existing.push(vol.name.clone());
            }
            Err(_) => {
                // Volume doesn't exist — try to create it.
                if let Some(ref container_name) = exec_container {
                    match relay_cmd(
                        target_host,
                        "docker.exec",
                        serde_json::json!({
                            "container": container_name,
                            "cmd": ["docker", "volume", "create", &vol.name],
                            "attach_stdout": true,
                            "attach_stderr": true,
                        }),
                        RELAY_TIMEOUT,
                    )
                    .await
                    {
                        Ok(resp) => {
                            let exit_code = resp
                                .payload
                                .get("exit_code")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(-1);
                            if exit_code == 0 {
                                tracing::info!(volume = %vol.name, "prepare: volume created on target");
                                volumes_created.push(vol.name.clone());
                            } else {
                                let warn = format!(
                                    "Volume create for '{}' returned exit code {}",
                                    vol.name, exit_code
                                );
                                tracing::warn!(%warn);
                                warnings.push(warn);
                            }
                        }
                        Err(e) => {
                            let warn = format!("Volume create failed for '{}': {}", vol.name, e);
                            tracing::warn!(%warn);
                            warnings.push(warn);
                        }
                    }
                } else {
                    let warn = format!(
                        "No running container found on target to create volume '{}'",
                        vol.name
                    );
                    tracing::warn!(%warn);
                    warnings.push(warn);
                }
            }
        }
    }

    // ── 5. Pre-flight checks ─────────────────────────────────────────

    // Verify all images that were attempted succeeded (no "failed" actions).
    let images_ok = !images.iter().any(|img| img.action == "failed");

    // Determine overall success: compose written, compose valid, images all ok.
    let success = compose_file_written && compose_valid && images_ok;

    let duration_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        success,
        compose_file_written,
        compose_valid,
        images = images.len(),
        volumes_created = volumes_created.len(),
        volumes_existing = volumes_existing.len(),
        warnings = warnings.len(),
        duration_ms,
        "prepare: complete"
    );

    Ok(PrepareResult {
        success,
        compose_file_written,
        compose_file_path: target_path,
        compose_valid,
        images,
        volumes_created,
        volumes_existing,
        warnings,
        duration_ms,
    })
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_compose_edits_env_changes() {
        let diff = ComposeDiff {
            env_changes: vec![super::super::analyze::EnvChange {
                key: "DB_HOST".into(),
                old: "localhost".into(),
                new: "192.168.1.59".into(),
            }],
            ..Default::default()
        };
        let content = "DB_HOST=localhost\nOTHER=keep";
        let result = apply_compose_edits(content, &diff);
        assert!(result.contains("DB_HOST=192.168.1.59"));
        assert!(result.contains("OTHER=keep"));
    }

    #[test]
    fn apply_compose_edits_volume_renames() {
        let mut diff = ComposeDiff::default();
        diff.volume_renames
            .insert("old_vol".into(), "new_vol".into());
        let content = "volumes:\n  - old_vol:/data";
        let result = apply_compose_edits(content, &diff);
        assert!(result.contains("new_vol"));
        assert!(!result.contains("old_vol"));
    }

    #[test]
    fn prepare_result_serde_roundtrip() {
        let result = PrepareResult {
            success: true,
            compose_file_written: true,
            compose_file_path: "/opt/stacks/test/docker-compose.yml".into(),
            compose_valid: true,
            images: vec![ImageResult {
                image: "nginx:latest".into(),
                action: "already_present".into(),
                architecture: "aarch64".into(),
            }],
            volumes_created: vec!["test_data".into()],
            volumes_existing: vec!["cache".into()],
            warnings: vec![],
            duration_ms: 1500,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: PrepareResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.images.len(), 1);
        assert_eq!(parsed.volumes_created.len(), 1);
    }
}
