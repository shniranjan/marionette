use serde::{Deserialize, Serialize};
use serde::ser::SerializeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerEndpoint {
    pub id: String,
    pub name: String,
    pub connection: String,
    #[serde(skip)]
    pub status: EndpointStatus,
    pub tags: Vec<String>,
    #[serde(default)]
    pub cert_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub enum EndpointStatus {
    Connected,
    #[default]
    Disconnected,
    Error(String),
}

impl Serialize for EndpointStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            EndpointStatus::Connected => s.serialize_str("connected"),
            EndpointStatus::Disconnected => s.serialize_str("disconnected"),
            EndpointStatus::Error(e) => {
                let mut map = s.serialize_map(Some(2))?;
                map.serialize_entry("status", "error")?;
                map.serialize_entry("message", e)?;
                map.end()
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointQuery {
    #[serde(default)]
    pub endpoint: Option<String>,
}
