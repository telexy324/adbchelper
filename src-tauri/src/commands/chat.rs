use rusqlite::Connection;
use tauri::State;

use crate::models::chat::{ChatMessage, ChatResponse, ChatSession, SendChatMessageInput, ToolDefinition};
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
