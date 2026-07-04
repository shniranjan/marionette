use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSummary {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    pub ports: Vec<PortMapping>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    pub private_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_port: Option<u16>,
    #[serde(rename = "type")]
    pub port_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDetail {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: String,
    pub platform: Option<String>,
    pub command: Option<String>,
    pub env: Vec<String>,
    pub ports: Vec<PortMapping>,
    pub mounts: Vec<Mount>,
    pub networks: Vec<ContainerNetwork>,
    pub restart_policy: Option<String>,
    pub labels: HashMap<String, String>,
    pub stack: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(rename = "type")]
    pub mount_type: String,
    pub source: String,
    pub destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerNetwork {
    pub name: String,
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
}
