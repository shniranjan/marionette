use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub id: String,
    pub path: String,
    pub target: String,
    pub auth_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_value: Option<String>,
    pub tls: bool,
    pub active: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteCreateRequest {
    pub path: String,
    pub target: String,
    #[serde(default = "default_auth_mode")]
    pub auth_mode: String,
    #[serde(default)]
    pub auth_value: Option<String>,
    #[serde(default)]
    pub tls: bool,
}

fn default_auth_mode() -> String { "none".to_string() }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteUpdateRequest {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub auth_mode: Option<String>,
    #[serde(default)]
    pub auth_value: Option<String>,
    #[serde(default)]
    pub tls: Option<bool>,
    #[serde(default)]
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteAccessRequest {
    pub user_id: String,
}
