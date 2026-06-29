use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub timestamp: String,
    pub action: String,
    pub endpoint_id: String,
    pub target: String,
    pub detail: String,
    pub admin_key_hash: String,
}
