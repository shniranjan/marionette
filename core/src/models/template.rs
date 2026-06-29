use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub image: String,
    #[serde(default = "default_ports")]
    pub ports: String,
    #[serde(default = "default_env_vars")]
    pub env_vars: String,
    #[serde(default = "default_volumes")]
    pub volumes: String,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: String,
    #[serde(default = "default_labels")]
    pub labels: String,
    pub created_at: String,
}

fn default_ports() -> String {
    "[]".to_string()
}
fn default_env_vars() -> String {
    "{}".to_string()
}
fn default_volumes() -> String {
    "[]".to_string()
}
fn default_restart_policy() -> String {
    "unless-stopped".to_string()
}
fn default_labels() -> String {
    "{}".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCreateRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub image: String,
    #[serde(default = "default_ports")]
    pub ports: String,
    #[serde(default = "default_env_vars")]
    pub env_vars: String,
    #[serde(default = "default_volumes")]
    pub volumes: String,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: String,
    #[serde(default = "default_labels")]
    pub labels: String,
}
