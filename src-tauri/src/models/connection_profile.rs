use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub id: String,
    pub environment_id: String,
    pub profile_type: String,
    pub name: String,
    pub endpoint: String,
    pub username: Option<String>,
    pub default_scope: Option<String>,
    pub notes: Option<String>,
    pub config_json: String,
    pub has_secret: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertEnvironmentInput {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub kubernetes_enabled: bool,
    pub elk_enabled: bool,
    pub ssh_enabled: bool,
    pub nacos_enabled: bool,
    pub redis_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertConnectionProfileInput {
    pub id: Option<String>,
    pub environment_id: String,
    pub profile_type: String,
    pub name: String,
    pub endpoint: String,
    pub username: Option<String>,
    pub default_scope: Option<String>,
    pub notes: Option<String>,
    pub config_json: Option<String>,
    pub secret_value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub ok: bool,
    pub messages: Vec<String>,
}
