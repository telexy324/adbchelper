use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListKubernetesEventsInput {
    pub environment_id: String,
    pub namespace: String,
    pub involved_object: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesEvent {
    pub id: String,
    pub namespace: String,
    pub kind: String,
    pub name: String,
    pub reason: String,
    pub level: String,
    pub message: String,
    pub event_time: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesEventsSummary {
    pub headline: String,
    pub likely_impact: Vec<String>,
    pub recommended_next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListKubernetesEventsResponse {
    pub environment_id: String,
    pub namespace: String,
    pub adapter_mode: String,
    pub query_summary: String,
    pub events: Vec<KubernetesEvent>,
    pub summary: KubernetesEventsSummary,
}
