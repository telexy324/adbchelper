use std::path::Path;

use chrono::Utc;
use rusqlite::Connection;
use serde_json::Value;

use crate::llm::qwen::{complete_chat, QwenConfig, QwenMessage};
use crate::models::investigation::{InvestigationCorrelation, InvestigationEvidence, InvestigationSummary};
use crate::models::chat::{ChatMessage, ChatResponse, ChatSession, SendChatMessageInput, ToolDefinition};
use crate::models::connection_profile::ConnectionProfile;
use crate::hardening::{sanitize_and_mask_json, sanitize_and_mask_text};
use crate::storage::{app_log, db, secrets};

pub fn tool_catalog() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_k8s_pods".to_string(),
            description: "Inspect workload status and restart counts for a namespace.".to_string(),
            input_hint: "{ environment, namespace, selector? }".to_string(),
        },
        ToolDefinition {
            name: "get_k8s_pod_logs".to_string(),
            description: "Read recent Kubernetes pod logs for troubleshooting.".to_string(),
            input_hint: "{ environment, namespace, podName, tailLines }".to_string(),
        },
        ToolDefinition {
            name: "list_k8s_events".to_string(),
            description: "Inspect Kubernetes warning and normal events for a namespace or workload.".to_string(),
            input_hint: "{ environment, namespace, involvedObject?, reason? }".to_string(),
        },
        ToolDefinition {
            name: "search_elk_logs".to_string(),
            description: "Search ELK for errors, keywords, or trace IDs.".to_string(),
            input_hint: "{ environment, service, timeRange, query }".to_string(),
        },
        ToolDefinition {
            name: "compare_nacos_config".to_string(),
            description: "Compare Nacos config across environments and highlight drift.".to_string(),
            input_hint: "{ sourceEnv, targetEnv, dataId, group, namespaceId? }".to_string(),
        },
        ToolDefinition {
            name: "inspect_ssh_host".to_string(),
            description: "Run approved read-only server diagnostics and review host logs.".to_string(),
            input_hint: "{ environment, host, commandPreset, logPath? }".to_string(),
        },
        ToolDefinition {
            name: "analyze_redis_instance".to_string(),
            description: "Review Redis INFO health, slowlog, latency trends, and Redis log warnings.".to_string(),
            input_hint: "{ environment, instanceName?, timeRange }".to_string(),
        },
        ToolDefinition {
            name: "analyze_tidb_slow_queries".to_string(),
            description: "Collect TiDB slow SQL rows, summarize hot digests, and prepare them for LLM analysis.".to_string(),
            input_hint: "{ environment, instanceName?, timeRange, slowQueryLimit? }".to_string(),
        },
    ]
}

pub async fn send_message(
    storage_path: &str,
    app_data_dir: &str,
    input: SendChatMessageInput,
) -> Result<ChatResponse, String> {
    let connection = Connection::open(storage_path).map_err(|error| error.to_string())?;
    let session = ensure_session(&connection, &input)?;
    let user_message = db::append_chat_message(&connection, &session.id, "user", input.content.trim(), None, None)
        .map_err(|error| error.to_string())?;

    db::insert_audit_log(&connection, Some(&session.id), Some(&input.environment_id), "user", "chat_message", None, None, Some(&user_message.content), "recorded")
        .map_err(|error| error.to_string())?;

    let recent_messages = db::list_chat_messages(&connection, &session.id).map_err(|error| error.to_string())?;
    let qwen_profile = select_qwen_profile(&connection, &input.environment_id)?;
    if !secrets::has_profile_secret(Some(Path::new(app_data_dir)), &qwen_profile.id) {
        let message = format!(
            "Qwen secret missing for profile '{}' ({}). Re-enter app_secret in Settings on this machine. Keychain entries do not move with the SQLite database.",
            qwen_profile.name, qwen_profile.id
        );
        let _ = app_log::append_log(Path::new(app_data_dir), "ERROR", "qwen_secret", &message);
        return Err(message);
    }
    let app_secret = secrets::get_profile_secret(Some(Path::new(app_data_dir)), &qwen_profile.id).map_err(|error| {
        let message = format!("Failed to load Qwen secret for profile '{}': {}", qwen_profile.name, error);
        let _ = app_log::append_log(Path::new(app_data_dir), "ERROR", "qwen_secret", &message);
        message
    })?;
    let qwen_config_json: Value =
        serde_json::from_str(&qwen_profile.config_json).unwrap_or_else(|_| Value::Object(Default::default()));
    let app_key = qwen_config_json
        .get("appKey")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            let message = format!(
                "Qwen app_key missing for profile '{}'. Fill in App key in Settings.",
                qwen_profile.name
            );
            let _ = app_log::append_log(Path::new(app_data_dir), "ERROR", "qwen_config", &message);
            message
        })?
        .to_string();
    let content_type = qwen_config_json
        .get("contentType")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("application/json")
        .to_string();
    let base_path = qwen_config_json
        .get("basePath")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("/chat/completions")
        .to_string();
    let model = qwen_profile
        .default_scope
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "qwen-plus".to_string());

    let investigation_context = build_investigation_context(&connection, &input)?;
    let system_prompt = build_system_prompt(&input.environment_id, investigation_context.as_deref());
    let qwen_messages = recent_messages_to_qwen_messages(&system_prompt, &recent_messages);

    db::insert_audit_log(&connection, Some(&session.id), Some(&input.environment_id), "assistant", "qwen_request", Some("qwen"), None, Some(&format!("model={model}, messages={}", qwen_messages.len())), "started")
        .map_err(|error| error.to_string())?;
    let _ = app_log::append_log(
        Path::new(app_data_dir),
        "INFO",
        "qwen_request",
        &format!(
            "session={} environment={} profile={} model={} endpoint={}",
            session.id, input.environment_id, qwen_profile.name, model, qwen_profile.endpoint
        ),
    );
    drop(connection);

    let completion = complete_chat(
        &QwenConfig {
            base_url: qwen_profile.endpoint.clone(),
            base_path,
            app_key,
            app_secret,
            model: model.clone(),
            content_type,
        },
        qwen_messages,
    )
    .await
    .map_err(|error| {
        let message = format!("Qwen request failed for profile '{}': {}", qwen_profile.name, error);
        let _ = app_log::append_log(Path::new(app_data_dir), "ERROR", "qwen_request", &message);
        message
    })?;
    let _ = app_log::append_log(
        Path::new(app_data_dir),
        "INFO",
        "qwen_response_raw",
        &completion.raw_body,
    );

    let connection = Connection::open(storage_path).map_err(|error| error.to_string())?;
    let assistant_message = db::append_chat_message(&connection, &session.id, "assistant", &completion.content, None, None)
        .map_err(|error| error.to_string())?;

    db::touch_chat_session(&connection, &session.id).map_err(|error| error.to_string())?;
    db::insert_audit_log(
        &connection,
        Some(&session.id),
        Some(&input.environment_id),
        "assistant",
        "qwen_response",
        Some("qwen"),
        None,
        Some(&assistant_message.content),
        "completed",
    )
    .map_err(|error| error.to_string())?;
    let _ = app_log::append_log(
        Path::new(app_data_dir),
        "INFO",
        "qwen_response",
        &format!("session={} environment={} response_saved=true", session.id, input.environment_id),
    );

    let session = db::get_chat_session(&connection, &session.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Chat session missing after save.".to_string())?;
    let messages = db::list_chat_messages(&connection, &session.id).map_err(|error| error.to_string())?;

    Ok(ChatResponse {
        session,
        messages,
        assistant_message,
        tool_catalog: tool_catalog(),
        model_used: model,
    })
}

fn ensure_session(connection: &Connection, input: &SendChatMessageInput) -> Result<ChatSession, String> {
    if let Some(session_id) = &input.session_id {
        return db::get_chat_session(connection, session_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Selected chat session no longer exists.".to_string());
    }

    let title = derive_title(&input.content);
    db::create_chat_session(connection, &input.environment_id, &title).map_err(|error| error.to_string())
}

pub fn ensure_tool_session(
    connection: &Connection,
    session_id: &Option<String>,
    environment_id: &str,
    title: &str,
) -> Result<ChatSession, String> {
    if let Some(session_id) = session_id {
        return db::get_chat_session(connection, session_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Selected chat session no longer exists.".to_string());
    }

    db::create_chat_session(connection, environment_id, title).map_err(|error| error.to_string())
}

fn derive_title(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return format!("Investigation {}", Utc::now().format("%Y-%m-%d %H:%M"));
    }

    let preview = trimmed.chars().take(48).collect::<String>();
    if trimmed.chars().count() > 48 {
        format!("{preview}...")
    } else {
        preview
    }
}

fn select_qwen_profile(connection: &Connection, environment_id: &str) -> Result<ConnectionProfile, String> {
    let profiles = db::list_connection_profiles(connection).map_err(|error| error.to_string())?;
    profiles
        .into_iter()
        .find(|profile| profile.environment_id == environment_id && profile.profile_type == "qwen")
        .ok_or_else(|| "No Qwen profile found for this environment. Add one in Settings first.".to_string())
}

fn build_system_prompt(environment_id: &str, investigation_context: Option<&str>) -> String {
    let tools = tool_catalog()
        .into_iter()
        .map(|tool| format!("- {}: {} {}", tool.name, tool.description, tool.input_hint))
        .collect::<Vec<_>>()
        .join("\n");

    let investigation_block = investigation_context
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("\nActive investigation context:\n{value}"))
        .unwrap_or_default();

    format!(
        "You are ADBCHelper, a desktop operations copilot.\nCurrent environment: {environment_id}.\nUse concise, evidence-driven answers.\nIf the user is asking to investigate something, mention which tool or data source should be used next from this catalog.\nTool catalog:\n{tools}{investigation_block}"
    )
}

fn recent_messages_to_qwen_messages(system_prompt: &str, messages: &[ChatMessage]) -> Vec<QwenMessage> {
    let mut converted = vec![QwenMessage {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    }];

    let recent = messages.iter().rev().take(8).cloned().collect::<Vec<_>>();
    for message in recent.into_iter().rev() {
        converted.push(QwenMessage {
            role: message.role,
            content: message.content,
        });
    }

    converted
}

fn build_investigation_context(
    connection: &Connection,
    input: &SendChatMessageInput,
) -> Result<Option<String>, String> {
    let Some(investigation_id) = input
        .investigation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let investigation = db::get_investigation(connection, investigation_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Selected investigation no longer exists.".to_string())?;

    if investigation.environment_id != input.environment_id {
        return Err(format!(
            "Investigation {} belongs to environment {}, not {}.",
            investigation.title, investigation.environment_id, input.environment_id
        ));
    }

    let mut evidence = db::list_investigation_evidence(connection, investigation_id)
        .map_err(|error| error.to_string())?;
    let selected_evidence_ids = input
        .selected_evidence_ids
        .as_ref()
        .map(|items| {
            items.iter()
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !selected_evidence_ids.is_empty() {
        evidence.retain(|item| selected_evidence_ids.iter().any(|selected| selected == &item.id));
        if evidence.is_empty() {
            return Err("None of the selected evidence items were found in this investigation.".to_string());
        }
    }

    let correlations = build_investigation_correlations(&evidence);

    Ok(Some(render_investigation_context(
        &investigation,
        &evidence,
        &correlations,
    )))
}

fn render_investigation_context(
    investigation: &InvestigationSummary,
    evidence: &[InvestigationEvidence],
    correlations: &[InvestigationCorrelation],
) -> String {
    let timeline = evidence
        .iter()
        .take(6)
        .map(|item| {
            format!(
                "- {} | {} | {}",
                item.created_at,
                sanitize_and_mask_text(&item.evidence_type),
                sanitize_and_mask_text(&item.title)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let evidence_blocks = evidence
        .iter()
        .take(4)
        .map(render_evidence_block)
        .collect::<Vec<_>>()
        .join("\n\n");

    let correlation_lines = if correlations.is_empty() {
        "- none".to_string()
    } else {
        correlations
            .iter()
            .take(4)
            .map(|item| {
                format!(
                    "- [{}] {}: {}",
                    sanitize_and_mask_text(&item.confidence),
                    sanitize_and_mask_text(&item.title),
                    sanitize_and_mask_text(&item.detail)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "Investigation: {title}\nStatus: {status}\nEvidence count: {count}\nRecent timeline:\n{timeline}\n\nCross-source correlation:\n{correlations}\n\nEvidence excerpts:\n{evidence_blocks}",
        title = sanitize_and_mask_text(&investigation.title),
        status = sanitize_and_mask_text(&investigation.status),
        count = evidence.len(),
        timeline = if timeline.is_empty() { "- none".to_string() } else { timeline },
        correlations = correlation_lines,
        evidence_blocks = if evidence_blocks.is_empty() {
            "- No evidence saved yet.".to_string()
        } else {
            evidence_blocks
        },
    )
}

fn render_evidence_block(item: &InvestigationEvidence) -> String {
    let compact_json = compact_json_for_model(&item.content_json, 900);
    format!(
        "Evidence: {title}\nType: {kind}\nSummary: {summary}\nContent:\n{content}",
        title = sanitize_and_mask_text(&item.title),
        kind = sanitize_and_mask_text(&item.evidence_type),
        summary = sanitize_and_mask_text(&item.summary),
        content = compact_json,
    )
}

fn compact_json_for_model(content_json: &str, max_chars: usize) -> String {
    let sanitized = sanitize_and_mask_json(content_json);
    let compact = match serde_json::from_str::<Value>(&sanitized) {
        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or(sanitized),
        Err(_) => sanitized,
    };

    if compact.chars().count() <= max_chars {
        compact
    } else {
        let truncated = compact.chars().take(max_chars).collect::<String>();
        format!("{truncated}\n...[truncated]")
    }
}

fn build_investigation_correlations(evidence: &[InvestigationEvidence]) -> Vec<InvestigationCorrelation> {
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
            {
                correlations.push(InvestigationCorrelation {
                    id: format!("{}-{}", k8s.id, nacos_item.id),
                    title: format!("Kubernetes events align with {} drift", data_id),
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
            let services = json_array_strings(&log_value, &["clusters"], &["services"]);

            if names.iter().any(|name| {
                let lowered_name = name.to_ascii_lowercase();
                services
                    .iter()
                    .any(|service| lowered_name.contains(&service.to_ascii_lowercase()))
            }) {
                correlations.push(InvestigationCorrelation {
                    id: format!("{}-{}", k8s.id, log_item.id),
                    title: format!("Kubernetes warnings align with {}", log_item.title),
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
        let message_blob = sanitize_and_mask_text(&log_item.summary).to_ascii_lowercase()
            + " "
            + &json_array_strings(&log_value, &["entries"], &["message"]).join(" ").to_ascii_lowercase();

        for ssh_item in &ssh {
            let ssh_blob = format!(
                "{} {}",
                sanitize_and_mask_text(&ssh_item.summary).to_ascii_lowercase(),
                ssh_item.content_json.to_ascii_lowercase()
            );
            if shared_keyword(&message_blob, &ssh_blob) {
                correlations.push(InvestigationCorrelation {
                    id: format!("{}-{}", log_item.id, ssh_item.id),
                    title: format!("{} aligns with host diagnostics", log_item.title),
                    detail: "Saved log evidence and SSH diagnostics point at the same service path or runtime symptom.".to_string(),
                    confidence: "medium".to_string(),
                    linked_evidence_ids: vec![log_item.id.clone(), ssh_item.id.clone()],
                });
            }
        }
    }

    correlations
}

fn parse_json(content_json: &str) -> Value {
    serde_json::from_str(content_json).unwrap_or_else(|_| Value::Object(Default::default()))
}

fn json_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    current.as_str().map(str::to_string)
}

fn json_array_strings(value: &Value, array_path: &[&str], field_path: &[&str]) -> Vec<String> {
    let mut current = value;
    for segment in array_path {
        current = match current.get(*segment) {
            Some(next) => next,
            None => return Vec::new(),
        };
    }

    current
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let mut nested = item;
            for segment in field_path {
                nested = nested.get(*segment)?;
            }
            nested.as_str().map(str::to_string)
        })
        .flat_map(|item| {
            item.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn shared_keyword(left: &str, right: &str) -> bool {
    ["timeout", "refused", "disk", "memory", "restart", "502", "504", "redis", "nginx"]
        .iter()
        .any(|keyword| left.contains(keyword) && right.contains(keyword))
}
