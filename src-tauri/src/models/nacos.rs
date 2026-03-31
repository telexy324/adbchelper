use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareNacosConfigInput {
    pub source_environment_id: String,
    pub target_environment_id: String,
    pub data_id: String,
    pub group: String,
    pub namespace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosConfigVersion {
    pub environment_id: String,
    pub profile_name: String,
    pub namespace_id: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosDiffEntry {
    pub key: String,
    pub status: String,
    pub source_value: Option<String>,
    pub target_value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosDiffSummary {
    pub headline: String,
    pub likely_impact: Vec<String>,
    pub explanation: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareNacosConfigResponse {
    pub source_environment_id: String,
    pub target_environment_id: String,
    pub data_id: String,
    pub group: String,
    pub namespace_id: Option<String>,
    pub adapter_mode: String,
    pub source: NacosConfigVersion,
    pub target: NacosConfigVersion,
    pub diff_entries: Vec<NacosDiffEntry>,
    pub summary: NacosDiffSummary,
}
