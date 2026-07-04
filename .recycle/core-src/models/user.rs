use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    Admin,
    Operator,
    Viewer,
}

impl UserRole {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Self::Admin),
            "operator" => Some(Self::Operator),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSummary {
    pub id: String,
    pub name: String,
    pub role: String,
    pub active: bool,
    pub created_at: String,
}
