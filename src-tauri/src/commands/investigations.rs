use rusqlite::Connection;
use tauri::State;

use crate::models::investigation::{
    InvestigationDetail, InvestigationEvidence, InvestigationReport, InvestigationReportInput,
    InvestigationSaveResponse, InvestigationSummary, InvestigationTimelineEvent, SaveInvestigationInput,
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

    Ok(InvestigationDetail {
        investigation,
        evidence,
        timeline,
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
        &input.summary,
        &input.content_json,
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

    let markdown = build_markdown_report(&investigation, &evidence, &timeline);
    let html = build_html_report(&investigation, &evidence, &timeline);

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
) -> String {
    let evidence_section = if evidence.is_empty() {
        "- No evidence saved yet.".to_string()
    } else {
        evidence
            .iter()
            .map(|item| {
                format!(
                    "### {}\n- Type: {}\n- Created: {}\n- Summary: {}\n\n```json\n{}\n```",
                    item.title, item.evidence_type, item.created_at, item.summary, pretty_json(&item.content_json)
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
            .map(|event| format!("- {} · {} · {}", event.timestamp, event.source_type, event.title))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "# {title}\n\n## Summary\n- Environment: {environment}\n- Status: {status}\n- Created: {created}\n- Updated: {updated}\n- Evidence count: {evidence_count}\n\n## Timeline\n{timeline}\n\n## Evidence\n{evidence}\n",
        title = investigation.title,
        environment = investigation.environment_id,
        status = investigation.status,
        created = investigation.created_at,
        updated = investigation.updated_at,
        evidence_count = evidence.len(),
        timeline = timeline_section,
        evidence = evidence_section,
    )
}

fn build_html_report(
    investigation: &InvestigationSummary,
    evidence: &[InvestigationEvidence],
    timeline: &[InvestigationTimelineEvent],
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
                    escape_html(&event.title),
                    escape_html(&event.detail)
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
                    escape_html(&item.summary),
                    escape_html(&pretty_json(&item.content_json))
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\" /><title>{title}</title><style>body{{font-family:Arial,sans-serif;margin:32px;background:#faf8f4;color:#1f2937}}h1,h2,h3{{margin-bottom:12px}}.meta{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;margin-bottom:24px}}.card,.evidence{{border:1px solid #e5e7eb;border-radius:16px;padding:16px;background:white;margin-bottom:16px}}pre{{white-space:pre-wrap;word-break:break-word;background:#f8fafc;padding:12px;border-radius:12px;overflow:auto}}</style></head><body><h1>{title}</h1><div class=\"meta\"><div class=\"card\"><strong>Environment</strong><div>{environment}</div></div><div class=\"card\"><strong>Status</strong><div>{status}</div></div><div class=\"card\"><strong>Created</strong><div>{created}</div></div><div class=\"card\"><strong>Updated</strong><div>{updated}</div></div></div><h2>Timeline</h2><div class=\"card\"><ul>{timeline}</ul></div><h2>Evidence</h2>{evidence}</body></html>",
        title = escape_html(&investigation.title),
        environment = escape_html(&investigation.environment_id),
        status = escape_html(&investigation.status),
        created = escape_html(&investigation.created_at),
        updated = escape_html(&investigation.updated_at),
        timeline = timeline_items,
        evidence = evidence_cards,
    )
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
