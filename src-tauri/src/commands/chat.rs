use rusqlite::Connection;
use tauri::State;

use crate::models::chat::{
    AttachToolEvidenceInput, ChatMessage, ChatResponse, ChatSession, SendChatMessageInput,
    ToolDefinition,
};
use crate::orchestrator::chat;
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_chat_sessions(state: State<'_, AppState>) -> Result<Vec<ChatSession>, String> {
    let connection = open_connection(&state.storage_path)?;
    db::list_chat_sessions(&connection).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_chat_messages(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<ChatMessage>, String> {
    let connection = open_connection(&state.storage_path)?;
    db::list_chat_messages(&connection, &session_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_tool_catalog() -> Vec<ToolDefinition> {
    chat::tool_catalog()
}

#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, AppState>,
    input: SendChatMessageInput,
) -> Result<ChatResponse, String> {
    chat::send_message(&state.storage_path, input).await
}

#[tauri::command]
pub fn attach_tool_evidence(
    state: State<'_, AppState>,
    input: AttachToolEvidenceInput,
) -> Result<ChatResponse, String> {
    let connection = open_connection(&state.storage_path)?;
    let session = chat::ensure_tool_session(&connection, &input.session_id, &input.environment_id, &input.title)?;
    let message = db::append_chat_message(
        &connection,
        &session.id,
        "tool",
        &input.content,
        Some(&input.tool_name),
        None,
    )
    .map_err(|error| error.to_string())?;
    db::touch_chat_session(&connection, &session.id).map_err(|error| error.to_string())?;
    db::insert_audit_log(
        &connection,
        Some(&session.id),
        Some(&input.environment_id),
        "user",
        "chat_tool_evidence",
        Some(&input.tool_name),
        None,
        Some(&message.content),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    let session = db::get_chat_session(&connection, &session.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Chat session missing after evidence attach.".to_string())?;
    let messages = db::list_chat_messages(&connection, &session.id).map_err(|error| error.to_string())?;

    Ok(ChatResponse {
        session,
        assistant_message: message,
        messages,
        tool_catalog: chat::tool_catalog(),
        model_used: "tool-evidence".to_string(),
    })
}
