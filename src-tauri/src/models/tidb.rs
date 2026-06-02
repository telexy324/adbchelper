use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeTidbInput {
    pub environment_id: String,
    pub instance_name: Option<String>,
    pub time_range: String,
    pub slow_query_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TidbSlowQuery {
    pub id: String,
    pub timestamp: String,
    pub query_time_secs: f64,
    pub digest: String,
    pub database_name: String,
    pub user: String,
    pub index_names: String,
    pub query_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TidbMetric {
    pub label: String,
    pub value: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TidbSummary {
    pub headline: String,
    pub likely_causes: Vec<String>,
    pub recommended_next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeTidbResponse {
    pub environment_id: String,
    pub instance_name: String,
    pub time_range: String,
    pub adapter_mode: String,
    pub executed_plan: String,
    pub source_relation: String,
    pub metrics: Vec<TidbMetric>,
    pub slow_queries: Vec<TidbSlowQuery>,
    pub summary: TidbSummary,
}
