use chrono::Utc;
use rusqlite::Connection;

use crate::llm::qwen::{complete_chat, QwenConfig, QwenMessage};
use crate::models::chat::{ChatMessage, ChatResponse, ChatSession, SendChatMessageInput, ToolDefinition};
use crate::models::connection_profile::ConnectionProfile;
use crate::storage::{db, secrets};

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
            name: "search_elk_logs".to_string(),
            description: "Search ELK for errors, keywords, or trace IDs.".to_string(),
            input_hint: "{ environment, service, timeRange, query }".to_string(),
        },
        ToolDefinition {
            name: "compare_nacos_config".to_string(),
            description: "Compare Nacos config across environments and highlight drift.".to_string(),
            input_hint: "{ sourceEnv, targetEnv, dataId, group }".to_string(),
        },
        ToolDefinition {
            name: "inspect_ssh_host".to_string(),
            description: "Run approved read-only server diagnostics and review host logs.".to_string(),
            input_hint: "{ environment, host, commandPreset, logPath? }".to_string(),
        },
    ]
}

pub async fn send_message(
    storage_path: &str,
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
    let api_key = secrets::get_profile_secret(&qwen_profile.id).map_err(|error| error.to_string())?;
    let model = qwen_profile
        .default_scope
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "qwen-plus".to_string());

    let system_prompt = build_system_prompt(&input.environment_id);
    let qwen_messages = recent_messages_to_qwen_messages(&system_prompt, &recent_messages);

    db::insert_audit_log(&connection, Some(&session.id), Some(&input.environment_id), "assistant", "qwen_request", Some("qwen"), None, Some(&format!("model={model}, messages={}", qwen_messages.len())), "started")
        .map_err(|error| error.to_string())?;
    drop(connection);

    let completion = complete_chat(
        &QwenConfig {
            base_url: qwen_profile.endpoint.clone(),
            api_key,
            model: model.clone(),
        },
        qwen_messages,
    )
    .await
    .map_err(|error| error.to_string())?;

    let connection = Connection::open(storage_path).map_err(|error| error.to_string())?;
    let assistant_message = db::append_chat_message(&connection, &session.id, "assistant", &completion, None, None)
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

fn build_system_prompt(environment_id: &str) -> String {
    let tools = tool_catalog()
        .into_iter()
        .map(|tool| format!("- {}: {} {}", tool.name, tool.description, tool.input_hint))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are ADBCHelper, a desktop operations copilot.\nCurrent environment: {environment_id}.\nUse concise, evidence-driven answers.\nIf the user is asking to investigate something, mention which tool or data source should be used next from this catalog.\nTool catalog:\n{tools}"
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
