use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSummary {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_human: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDeepInspection {
    pub name: String,
    pub driver: String,
    pub driver_type: String,
    pub driver_category: String,
    pub migration_advice: String,
    pub mountpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_human: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    pub used_by: Vec<String>,
    pub shared: bool,
    pub options: HashMap<String, String>,
    pub options_sanitized: HashMap<String, String>,
    pub labels: HashMap<String, String>,
    pub scope: String,
    pub needs_chown: bool,
    pub mount_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCreateRequest {
    pub name: String,
    #[serde(default)]
    pub driver: Option<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub options: HashMap<String, String>,
}
