use rusqlite::Connection;
use tauri::State;

use crate::hardening::{sanitize_and_mask_json, sanitize_and_mask_text};
use crate::models::investigation::{
    InvestigationCorrelation, InvestigationDetail, InvestigationEvidence, InvestigationReport,
    InvestigationReportInput, InvestigationSaveResponse, InvestigationSummary,
    InvestigationTimelineEvent, SaveInvestigationInput,
};
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_investigations(state: State<'_, AppState>) -> Result<Vec<InvestigationSummary>, String> {
    let connection = open_connection(&state.storage_path)?;
    db::list_investigations(&connection).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_investigation_evidence(
    state: State<'_, AppState>,
    investigation_id: String,
) -> Result<Vec<InvestigationEvidence>, String> {
    let connection = open_connection(&state.storage_path)?;
    db::list_investigation_evidence(&connection, &investigation_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_investigation_detail(
    state: State<'_, AppState>,
    investigation_id: String,
) -> Result<InvestigationDetail, String> {
    let connection = open_connection(&state.storage_path)?;
    let investigation = db::get_investigation(&connection, &investigation_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Selected investigation no longer exists.".to_string())?;
    let evidence = db::list_investigation_evidence(&connection, &investigation_id)
        .map_err(|error| error.to_string())?;
    let timeline = build_timeline(&evidence);
    let correlations = build_correlations(&evidence);

    Ok(InvestigationDetail {
        investigation,
        evidence,
        timeline,
        correlations,
    })
}

#[tauri::command]
pub fn save_investigation_evidence(
    state: State<'_, AppState>,
    input: SaveInvestigationInput,
) -> Result<InvestigationSaveResponse, String> {
    let connection = open_connection(&state.storage_path)?;

    let investigation = match input.investigation_id.as_deref() {
        Some(investigation_id) => db::get_investigation(&connection, investigation_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Selected investigation no longer exists.".to_string())?,
        None => db::create_investigation(
            &connection,
            &input.environment_id,
            input
                .title
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("Investigation"),
        )
        .map_err(|error| error.to_string())?,
    };

    let evidence = db::add_investigation_evidence(
        &connection,
        &investigation.id,
        &input.evidence_type,
        &input.evidence_title,
        &sanitize_and_mask_text(&input.summary),
        &sanitize_and_mask_json(&input.content_json),
    )
    .map_err(|error| error.to_string())?;

    db::touch_investigation(&connection, &investigation.id).map_err(|error| error.to_string())?;
    let investigation = db::get_investigation(&connection, &investigation.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Investigation missing after save.".to_string())?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&investigation.environment_id),
        "user",
        "investigation_evidence_save",
        Some(&input.evidence_type),
        Some(&investigation.id),
        Some(&input.evidence_title),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(InvestigationSaveResponse {
        investigation,
        evidence,
    })
}

#[tauri::command]
pub fn generate_investigation_report(
    state: State<'_, AppState>,
    input: InvestigationReportInput,
) -> Result<InvestigationReport, String> {
    let connection = open_connection(&state.storage_path)?;
    let investigation = db::get_investigation(&connection, &input.investigation_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Selected investigation no longer exists.".to_string())?;
    let evidence = db::list_investigation_evidence(&connection, &input.investigation_id)
        .map_err(|error| error.to_string())?;
    let timeline = build_timeline(&evidence);
    let correlations = build_correlations(&evidence);

    let markdown = build_markdown_report(&investigation, &evidence, &timeline, &correlations);
    let html = build_html_report(&investigation, &evidence, &timeline, &correlations);

    db::insert_audit_log(
        &connection,
        None,
        Some(&investigation.environment_id),
        "user",
        "investigation_report_generate",
        Some("report"),
        Some(&investigation.id),
        Some(&investigation.title),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(InvestigationReport {
        investigation,
        markdown,
        html,
    })
}

fn build_timeline(evidence: &[InvestigationEvidence]) -> Vec<InvestigationTimelineEvent> {
    let mut timeline = evidence
        .iter()
        .map(|item| InvestigationTimelineEvent {
            id: item.id.clone(),
            timestamp: item.created_at.clone(),
            title: item.title.clone(),
            detail: item.summary.clone(),
            source_type: item.evidence_type.clone(),
        })
        .collect::<Vec<_>>();
    timeline.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
    timeline
}

fn build_markdown_report(
    investigation: &InvestigationSummary,
    evidence: &[InvestigationEvidence],
    timeline: &[InvestigationTimelineEvent],
    correlations: &[InvestigationCorrelation],
) -> String {
    let evidence_section = if evidence.is_empty() {
        "- No evidence saved yet.".to_string()
    } else {
        evidence
            .iter()
            .map(|item| {
                format!(
                    "### {}\n- Type: {}\n- Created: {}\n- Summary: {}\n\n```json\n{}\n```",
                    sanitize_and_mask_text(&item.title),
                    sanitize_and_mask_text(&item.evidence_type),
                    item.created_at,
                    sanitize_and_mask_text(&item.summary),
                    pretty_json(&item.content_json)
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let timeline_section = if timeline.is_empty() {
        "- No timeline events yet.".to_string()
    } else {
        timeline
            .iter()
            .map(|event| format!("- {} · {} · {}", event.timestamp, sanitize_and_mask_text(&event.source_type), sanitize_and_mask_text(&event.title)))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let correlation_section = if correlations.is_empty() {
        "- No cross-source correlations detected yet.".to_string()
    } else {
        correlations
            .iter()
            .map(|item| format!("- [{}] {}: {}", sanitize_and_mask_text(&item.confidence), sanitize_and_mask_text(&item.title), sanitize_and_mask_text(&item.detail)))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "# {title}\n\n## Summary\n- Environment: {environment}\n- Status: {status}\n- Created: {created}\n- Updated: {updated}\n- Evidence count: {evidence_count}\n\n## Timeline\n{timeline}\n\n## Cross-Source Correlation\n{correlations}\n\n## Evidence\n{evidence}\n",
        title = investigation.title,
        environment = investigation.environment_id,
        status = sanitize_and_mask_text(&investigation.status),
        created = investigation.created_at,
        updated = investigation.updated_at,
        evidence_count = evidence.len(),
        timeline = timeline_section,
        correlations = correlation_section,
        evidence = evidence_section,
    )
}

fn build_html_report(
    investigation: &InvestigationSummary,
    evidence: &[InvestigationEvidence],
    timeline: &[InvestigationTimelineEvent],
    correlations: &[InvestigationCorrelation],
) -> String {
    let timeline_items = if timeline.is_empty() {
        "<li>No timeline events yet.</li>".to_string()
    } else {
        timeline
            .iter()
            .map(|event| {
                format!(
                    "<li><strong>{}</strong> <span>{}</span><div>{}</div></li>",
                    escape_html(&event.timestamp),
                    escape_html(&sanitize_and_mask_text(&event.title)),
                    escape_html(&sanitize_and_mask_text(&event.detail))
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    let evidence_cards = if evidence.is_empty() {
        "<p>No evidence saved yet.</p>".to_string()
    } else {
        evidence
            .iter()
            .map(|item| {
                format!(
                    "<section class=\"evidence\"><h3>{}</h3><p>{}</p><pre>{}</pre></section>",
                    escape_html(&item.title),
                    escape_html(&sanitize_and_mask_text(&item.summary)),
                    escape_html(&pretty_json(&item.content_json))
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    let correlation_cards = if correlations.is_empty() {
        "<p>No cross-source correlations detected yet.</p>".to_string()
    } else {
        correlations
            .iter()
            .map(|item| {
                format!(
                    "<section class=\"card\"><h3>{}</h3><p><strong>{}</strong></p><p>{}</p></section>",
                    escape_html(&item.title),
                    escape_html(&sanitize_and_mask_text(&item.confidence)),
                    escape_html(&sanitize_and_mask_text(&item.detail))
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\" /><title>{title}</title><style>body{{font-family:Arial,sans-serif;margin:32px;background:#faf8f4;color:#1f2937}}h1,h2,h3{{margin-bottom:12px}}.meta{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;margin-bottom:24px}}.card,.evidence{{border:1px solid #e5e7eb;border-radius:16px;padding:16px;background:white;margin-bottom:16px}}pre{{white-space:pre-wrap;word-break:break-word;background:#f8fafc;padding:12px;border-radius:12px;overflow:auto}}</style></head><body><h1>{title}</h1><div class=\"meta\"><div class=\"card\"><strong>Environment</strong><div>{environment}</div></div><div class=\"card\"><strong>Status</strong><div>{status}</div></div><div class=\"card\"><strong>Created</strong><div>{created}</div></div><div class=\"card\"><strong>Updated</strong><div>{updated}</div></div></div><h2>Timeline</h2><div class=\"card\"><ul>{timeline}</ul></div><h2>Cross-Source Correlation</h2>{correlations}<h2>Evidence</h2>{evidence}</body></html>",
        title = escape_html(&investigation.title),
        environment = escape_html(&investigation.environment_id),
        status = escape_html(&investigation.status),
        created = escape_html(&investigation.created_at),
        updated = escape_html(&investigation.updated_at),
        timeline = timeline_items,
        correlations = correlation_cards,
        evidence = evidence_cards,
    )
}

fn build_correlations(evidence: &[InvestigationEvidence]) -> Vec<InvestigationCorrelation> {
    let mut correlations = Vec::new();

    let kubernetes = evidence
        .iter()
        .filter(|item| item.evidence_type == "kubernetes_events")
        .collect::<Vec<_>>();
    let nacos = evidence
        .iter()
        .filter(|item| item.evidence_type == "nacos_diff")
        .collect::<Vec<_>>();
    let logs = evidence
        .iter()
        .filter(|item| item.evidence_type == "log_search")
        .collect::<Vec<_>>();
    let ssh = evidence
        .iter()
        .filter(|item| item.evidence_type == "ssh_diagnostics")
        .collect::<Vec<_>>();

    for k8s in &kubernetes {
        let k8s_value = parse_json(&k8s.content_json);
        let namespace = json_string(&k8s_value, &["namespace"]).unwrap_or_else(|| "unknown".to_string());
        let names = json_array_strings(&k8s_value, &["events"], &["name"]);

        for nacos_item in &nacos {
            let nacos_value = parse_json(&nacos_item.content_json);
            let data_id = json_string(&nacos_value, &["dataId"]).unwrap_or_else(|| nacos_item.title.clone());
            let inferred_service = data_id.split('.').next().unwrap_or(&data_id).to_ascii_lowercase();
            if names
                .iter()
                .any(|name| name.to_ascii_lowercase().contains(&inferred_service))
                || nacos_item.summary.to_ascii_lowercase().contains(&namespace.to_ascii_lowercase())
            {
                correlations.push(InvestigationCorrelation {
                    id: format!("{}-{}", k8s.id, nacos_item.id),
                    title: "Kubernetes events align with config drift".to_string(),
                    detail: format!(
                        "Events in namespace {} mention workloads related to {}, which also appears in the saved Nacos drift evidence.",
                        namespace, data_id
                    ),
                    confidence: "medium".to_string(),
                    linked_evidence_ids: vec![k8s.id.clone(), nacos_item.id.clone()],
                });
            }
        }

        for log_item in &logs {
            let log_value = parse_json(&log_item.content_json);
            let services = json_array_strings(&log_value, &["entries"], &["service"]);
            if names.iter().any(|name| services.iter().any(|service| name.to_ascii_lowercase().contains(&service.to_ascii_lowercase()))) {
                correlations.push(InvestigationCorrelation {
                    id: format!("{}-{}", k8s.id, log_item.id),
                    title: "Kubernetes warnings overlap with log services".to_string(),
                    detail: format!(
                        "Saved Kubernetes events and log evidence reference overlapping workloads in namespace {}.",
                        namespace
                    ),
                    confidence: "high".to_string(),
                    linked_evidence_ids: vec![k8s.id.clone(), log_item.id.clone()],
                });
            }
        }
    }

    for log_item in &logs {
        let log_value = parse_json(&log_item.content_json);
        let services = json_array_strings(&log_value, &["entries"], &["service"]);
        for ssh_item in &ssh {
            let ssh_value = parse_json(&ssh_item.content_json);
            let summary = json_string(&ssh_value, &["summaryHeadline"]).unwrap_or_else(|| ssh_item.summary.clone());
            if services
                .iter()
                .any(|service| summary.to_ascii_lowercase().contains(&service.to_ascii_lowercase()))
            {
                correlations.push(InvestigationCorrelation {
                    id: format!("{}-{}", log_item.id, ssh_item.id),
                    title: "Application logs align with host diagnostics".to_string(),
                    detail: "Saved log evidence and SSH diagnostics point at the same service path or runtime symptom.".to_string(),
                    confidence: "medium".to_string(),
                    linked_evidence_ids: vec![log_item.id.clone(), ssh_item.id.clone()],
                });
            }
        }
    }

    correlations.sort_by(|left, right| left.title.cmp(&right.title));
    correlations.dedup_by(|left, right| left.id == right.id);
    correlations
}

fn parse_json(content: &str) -> serde_json::Value {
    serde_json::from_str(content).unwrap_or(serde_json::Value::Null)
}

fn json_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(str::to_string)
}

fn json_array_strings(
    value: &serde_json::Value,
    array_path: &[&str],
    leaf_path: &[&str],
) -> Vec<String> {
    let mut current = value;
    for segment in array_path {
        current = match current.get(*segment) {
            Some(next) => next,
            None => return Vec::new(),
        };
    }

    current
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| json_string(item, leaf_path))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn pretty_json(content_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(content_json)
        .map(|value| serde_json::to_string_pretty(&value).unwrap_or_else(|_| content_json.to_string()))
        .unwrap_or_else(|_| content_json.to_string())
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
