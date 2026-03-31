use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationSummary {
    pub id: String,
    pub title: String,
    pub environment_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationEvidence {
    pub id: String,
    pub investigation_id: String,
    pub evidence_type: String,
    pub title: String,
    pub summary: String,
    pub content_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveInvestigationInput {
    pub investigation_id: Option<String>,
    pub title: Option<String>,
    pub environment_id: String,
    pub evidence_type: String,
    pub evidence_title: String,
    pub summary: String,
    pub content_json: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestigationSaveResponse {
    pub investigation: InvestigationSummary,
    pub evidence: InvestigationEvidence,
}
