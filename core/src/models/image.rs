use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSummary {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size: i64,
    pub created: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDetail {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size: i64,
    pub created: String,
    pub os: Option<String>,
    pub architecture: Option<String>,
    pub layers: Vec<ImageLayer>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLayer {
    pub id: String,
    pub size: i64,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePullRequest {
    pub image: String,
    #[serde(default)]
    pub tag: Option<String>,
}
