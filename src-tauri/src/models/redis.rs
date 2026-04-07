use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRedisInput {
    pub environment_id: String,
    pub instance_name: Option<String>,
    pub time_range: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisInfoMetric {
    pub label: String,
    pub value: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisSlowQuery {
    pub id: String,
    pub timestamp: String,
    pub duration_micros: u64,
    pub command: String,
    pub key_sample: String,
    pub client: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisLatencyPoint {
    pub timestamp: String,
    pub avg_ms: f64,
    pub p95_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisLogLine {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisSummary {
    pub headline: String,
    pub likely_causes: Vec<String>,
    pub recommended_next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRedisResponse {
    pub environment_id: String,
    pub instance_name: String,
    pub time_range: String,
    pub adapter_mode: String,
    pub executed_plan: String,
    pub info_metrics: Vec<RedisInfoMetric>,
    pub slow_queries: Vec<RedisSlowQuery>,
    pub latency_points: Vec<RedisLatencyPoint>,
    pub log_lines: Vec<RedisLogLine>,
    pub summary: RedisSummary,
}
