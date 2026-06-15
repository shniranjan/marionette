use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use bollard::image::{ListImagesOptions, RemoveImageOptions};
use futures::StreamExt;
use std::sync::Arc;

use crate::docker::*;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── List Images ───────────────────────────────────────────────

pub async fn list_images(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<ImageSummary>> {
    let endpoint_id = params
        .endpoint
        .unwrap_or_else(|| state.default_endpoint.clone());
    let clients = state.clients.read().await;
    let docker = get_client(&endpoint_id, &clients)
        .await
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    drop(clients);

    let images = docker
        .list_images(Some(ListImagesOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // ImageSummary fields in bollard 0.17 are plain types: id:String, repo_tags:Vec<String>, size:i64, created:i64
    let summaries: Vec<ImageSummary> = images
        .into_iter()
        .map(|img| ImageSummary {
            id: img.id,
            repo_tags: img.repo_tags,
            size: img.size,
            created: img.created,
        })
        .collect();

    Ok(Json(summaries))
}

// ── Inspect Image ─────────────────────────────────────────────

pub async fn inspect_image(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<ImageDetail> {
    let endpoint_id = params
        .endpoint
        .unwrap_or_else(|| state.default_endpoint.clone());
    let clients = state.clients.read().await;
    let docker = get_client(&endpoint_id, &clients)
        .await
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    drop(clients);

    let info = docker
        .inspect_image(&id)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))?;

    // Get layer IDs from root_fs.layers
    let layer_ids = info
        .root_fs
        .as_ref()
        .and_then(|rfs| rfs.layers.clone())
        .unwrap_or_default();

    // Get layer history (size + comment) from the image_history API
    let history_entries = docker
        .image_history(&id)
        .await
        .unwrap_or_default();

    let layers: Vec<ImageLayer> = layer_ids
        .into_iter()
        .enumerate()
        .map(|(i, layer_id)| {
            let (layer_size, comment) = history_entries
                .get(i)
                .map(|h| (h.size, Some(h.comment.clone())))
                .unwrap_or((0, None));
            ImageLayer {
                id: layer_id,
                size: layer_size,
                comment,
            }
        })
        .collect();

    let detail = ImageDetail {
        id: info.id.unwrap_or_default(),
        repo_tags: info.repo_tags.unwrap_or_default(),
        size: info.size.unwrap_or(0),
        created: info.created.unwrap_or_default(),
        os: info.os.clone(),
        architecture: info.architecture.clone(),
        layers,
    };

    Ok(Json(detail))
}

// ── Pull Image ────────────────────────────────────────────────

pub async fn pull_image(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<ImagePullRequest>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = params
        .endpoint
        .unwrap_or_else(|| state.default_endpoint.clone());
    let clients = state.clients.read().await;
    let docker = get_client(&endpoint_id, &clients)
        .await
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    drop(clients);

    let image_ref = match &body.tag {
        Some(tag) => format!("{}:{}", body.image, tag),
        None => body.image.clone(),
    };

    let options = bollard::image::CreateImageOptions {
        from_image: body.image.clone(),
        tag: body.tag.clone().unwrap_or_else(|| "latest".to_string()),
        ..Default::default()
    };

    let mut stream = docker.create_image(
        Some(options),
        None,
        None,
    );

    let mut progress_lines: Vec<String> = Vec::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(info) => {
                if let Some(status) = &info.status {
                    let line = if let Some(pfx) = &info.id {
                        format!("{}: {}", pfx, status)
                    } else if let Some(progress) = &info.progress {
                        format!("{}: {}", status, progress)
                    } else {
                        status.clone()
                    };
                    progress_lines.push(line);
                }
                if let Some(error_msg) = &info.error {
                    return Err(error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Pull error: {}", error_msg),
                    ));
                }
            }
            Err(e) => {
                return Err(error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &e.to_string(),
                ));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "status": "pulled",
        "image": image_ref,
        "progress": progress_lines
    })))
}

// ── Remove Image ──────────────────────────────────────────────

pub async fn remove_image(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = params
        .endpoint
        .unwrap_or_else(|| state.default_endpoint.clone());
    let clients = state.clients.read().await;
    let docker = get_client(&endpoint_id, &clients)
        .await
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    drop(clients);

    let results = docker
        .remove_image(
            &id,
            Some(RemoveImageOptions {
                force: true,
                ..Default::default()
            }),
            None,
        )
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let deleted: Vec<String> = results
        .into_iter()
        .filter_map(|r| r.untagged.or(r.deleted))
        .collect();

    Ok(Json(serde_json::json!({
        "status": "removed",
        "id": id,
        "deleted": deleted
    })))
}

// ── Image History ─────────────────────────────────────────────

pub async fn image_history(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<ImageLayer>> {
    let endpoint_id = params
        .endpoint
        .unwrap_or_else(|| state.default_endpoint.clone());
    let clients = state.clients.read().await;
    let docker = get_client(&endpoint_id, &clients)
        .await
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    drop(clients);

    let history = docker
        .image_history(&id)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // Bollard 0.17: h.id is String, h.size is i64, h.comment is String
    let layers: Vec<ImageLayer> = history
        .into_iter()
        .map(|h| ImageLayer {
            id: h.id,
            size: h.size,
            comment: Some(h.comment),
        })
        .collect();

    Ok(Json(layers))
}
