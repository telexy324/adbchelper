use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    pub environment_id: String,
    pub action_type: String,
    pub target_ref: String,
    pub status: String,
    pub risk_level: String,
    pub rationale: String,
    pub rollback_hint: String,
    pub execution_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApprovalInput {
    pub environment_id: String,
    pub action_type: String,
    pub target_ref: String,
    pub target_details_json: String,
    pub rationale: String,
    pub rollback_hint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteApprovalInput {
    pub approval_id: String,
}
