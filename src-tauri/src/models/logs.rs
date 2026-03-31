use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSearchInput {
    pub environment_id: String,
    pub service: Option<String>,
    pub pod: Option<String>,
    pub keyword: Option<String>,
    pub trace_id: Option<String>,
    pub time_range: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub timestamp: String,
    pub environment_id: String,
    pub service: String,
    pub pod: String,
    pub level: String,
    pub trace_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogCluster {
    pub id: String,
    pub label: String,
    pub level: String,
    pub count: usize,
    pub services: Vec<String>,
    pub example_message: String,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSummary {
    pub headline: String,
    pub likely_causes: Vec<String>,
    pub recommended_next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSearchResponse {
    pub environment_id: String,
    pub time_range: String,
    pub adapter_mode: String,
    pub executed_query: String,
    pub entries: Vec<LogEntry>,
    pub clusters: Vec<LogCluster>,
    pub summary: LogSummary,
}
