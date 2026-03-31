use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDiagnosticsInput {
    pub environment_id: String,
    pub host: Option<String>,
    pub command_preset: String,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHealthMetric {
    pub label: String,
    pub status: String,
    pub value: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshLogLine {
    pub timestamp: String,
    pub source: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDiagnosticsResponse {
    pub environment_id: String,
    pub adapter_mode: String,
    pub target_host: String,
    pub command_preset: String,
    pub executed_command: String,
    pub allowed_commands: Vec<String>,
    pub health_summary: Vec<SshHealthMetric>,
    pub log_lines: Vec<SshLogLine>,
    pub summary_headline: String,
    pub recommended_actions: Vec<String>,
}
