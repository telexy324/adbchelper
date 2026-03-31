use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection};
use thiserror::Error;

use crate::models::connection_profile::{
    ConnectionProfile, UpsertConnectionProfileInput, UpsertEnvironmentInput, ValidationResult,
};
use crate::models::chat::{ChatMessage, ChatSession};
use crate::models::approval::ApprovalRequest;
use crate::models::environment::EnvironmentProfile;
use crate::models::investigation::{InvestigationEvidence, InvestigationSummary};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to create app data directory: {0}")]
    CreateDirectory(#[from] std::io::Error),
    #[error("failed to open sqlite database: {0}")]
    OpenDatabase(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct DatabaseStatus {
    pub storage_path: PathBuf,
    pub database_ready: bool,
}

pub fn initialize_database(base_dir: &Path) -> Result<DatabaseStatus, StorageError> {
    fs::create_dir_all(base_dir)?;

    let database_path = base_dir.join("adbchelper.db");
    let connection = Connection::open(&database_path)?;

    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS app_metadata (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS environments (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          kind TEXT NOT NULL,
          kubernetes_enabled INTEGER NOT NULL DEFAULT 0,
          elk_enabled INTEGER NOT NULL DEFAULT 0,
          ssh_enabled INTEGER NOT NULL DEFAULT 0,
          nacos_enabled INTEGER NOT NULL DEFAULT 0,
          redis_enabled INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS connection_profiles (
          id TEXT PRIMARY KEY,
          environment_id TEXT NOT NULL,
          profile_type TEXT NOT NULL,
          name TEXT NOT NULL,
          endpoint TEXT NOT NULL DEFAULT '',
          username TEXT,
          default_scope TEXT,
          notes TEXT,
          config_json TEXT NOT NULL DEFAULT '{}',
          has_secret INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(environment_id) REFERENCES environments(id)
        );
        CREATE TABLE IF NOT EXISTS chat_sessions (
          id TEXT PRIMARY KEY,
          environment_id TEXT NOT NULL,
          title TEXT NOT NULL,
          status TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(environment_id) REFERENCES environments(id)
        );
        CREATE TABLE IF NOT EXISTS chat_messages (
          id TEXT PRIMARY KEY,
          session_id TEXT NOT NULL,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          tool_name TEXT,
          tool_call_id TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(session_id) REFERENCES chat_sessions(id)
        );
        CREATE TABLE IF NOT EXISTS audit_logs (
          id TEXT PRIMARY KEY,
          session_id TEXT,
          environment_id TEXT,
          actor_type TEXT NOT NULL,
          event_type TEXT NOT NULL,
          tool_name TEXT,
          target_ref TEXT,
          request_json TEXT,
          status TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS investigations (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          environment_id TEXT NOT NULL,
          status TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(environment_id) REFERENCES environments(id)
        );
        CREATE TABLE IF NOT EXISTS investigation_evidence (
          id TEXT PRIMARY KEY,
          investigation_id TEXT NOT NULL,
          evidence_type TEXT NOT NULL,
          title TEXT NOT NULL,
          summary TEXT NOT NULL,
          content_json TEXT NOT NULL,
          created_at TEXT NOT NULL,
          FOREIGN KEY(investigation_id) REFERENCES investigations(id)
        );
        CREATE TABLE IF NOT EXISTS approval_requests (
          id TEXT PRIMARY KEY,
          environment_id TEXT NOT NULL,
          action_type TEXT NOT NULL,
          target_ref TEXT NOT NULL,
          target_details_json TEXT NOT NULL,
          status TEXT NOT NULL,
          risk_level TEXT NOT NULL,
          rationale TEXT NOT NULL,
          rollback_hint TEXT NOT NULL,
          execution_summary TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(environment_id) REFERENCES environments(id)
        );
        "#
    )?;

    seed_environments(&connection)?;

    Ok(DatabaseStatus {
        storage_path: database_path,
        database_ready: true,
    })
}

fn seed_environments(connection: &Connection) -> Result<(), rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let seeded_rows = [
        ("dev", "Development", "dev"),
        ("test", "Testing", "test"),
        ("prod", "Production", "prod"),
    ];

    for (id, name, kind) in seeded_rows {
        connection.execute(
            r#"
            INSERT OR IGNORE INTO environments (
              id,
              name,
              kind,
              kubernetes_enabled,
              elk_enabled,
              ssh_enabled,
              nacos_enabled,
              redis_enabled
            ) VALUES (?1, ?2, ?3, 1, 1, 1, 1, 1)
            "#,
            params![id, name, kind],
        )?;

        connection.execute(
            "INSERT OR REPLACE INTO app_metadata (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![format!("seeded_environment:{id}"), name, now],
        )?;
    }

    Ok(())
}

pub fn list_environments(connection: &Connection) -> Result<Vec<EnvironmentProfile>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT
          id,
          name,
          kind,
          kubernetes_enabled,
          elk_enabled,
          ssh_enabled,
          nacos_enabled,
          redis_enabled
        FROM environments
        ORDER BY CASE kind
          WHEN 'dev' THEN 1
          WHEN 'test' THEN 2
          WHEN 'prod' THEN 3
          ELSE 4
        END
        "#,
    )?;

    let environment_rows = statement.query_map([], |row| {
        Ok(EnvironmentProfile {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            kubernetes_enabled: row.get(3)?,
            elk_enabled: row.get(4)?,
            ssh_enabled: row.get(5)?,
            nacos_enabled: row.get(6)?,
            redis_enabled: row.get(7)?,
        })
    })?;

    environment_rows.collect::<Result<Vec<_>, _>>()
}

pub fn upsert_environment(
    connection: &Connection,
    input: UpsertEnvironmentInput,
) -> Result<EnvironmentProfile, rusqlite::Error> {
    connection.execute(
        r#"
        INSERT INTO environments (
          id,
          name,
          kind,
          kubernetes_enabled,
          elk_enabled,
          ssh_enabled,
          nacos_enabled,
          redis_enabled
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(id) DO UPDATE SET
          name = excluded.name,
          kind = excluded.kind,
          kubernetes_enabled = excluded.kubernetes_enabled,
          elk_enabled = excluded.elk_enabled,
          ssh_enabled = excluded.ssh_enabled,
          nacos_enabled = excluded.nacos_enabled,
          redis_enabled = excluded.redis_enabled
        "#,
        params![
            input.id,
            input.name,
            input.kind,
            input.kubernetes_enabled,
            input.elk_enabled,
            input.ssh_enabled,
            input.nacos_enabled,
            input.redis_enabled
        ],
    )?;

    Ok(EnvironmentProfile {
        id: input.id,
        name: input.name,
        kind: input.kind,
        kubernetes_enabled: input.kubernetes_enabled,
        elk_enabled: input.elk_enabled,
        ssh_enabled: input.ssh_enabled,
        nacos_enabled: input.nacos_enabled,
        redis_enabled: input.redis_enabled,
    })
}

pub fn list_connection_profiles(
    connection: &Connection,
) -> Result<Vec<ConnectionProfile>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT
          id,
          environment_id,
          profile_type,
          name,
          endpoint,
          username,
          default_scope,
          notes,
          config_json,
          has_secret,
          created_at,
          updated_at
        FROM connection_profiles
        ORDER BY environment_id, profile_type, name
        "#,
    )?;

    let rows = statement.query_map([], |row| {
        Ok(ConnectionProfile {
            id: row.get(0)?,
            environment_id: row.get(1)?,
            profile_type: row.get(2)?,
            name: row.get(3)?,
            endpoint: row.get(4)?,
            username: row.get(5)?,
            default_scope: row.get(6)?,
            notes: row.get(7)?,
            config_json: row.get(8)?,
            has_secret: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

pub fn get_connection_profile(
    connection: &Connection,
    profile_id: &str,
) -> Result<Option<ConnectionProfile>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT
          id,
          environment_id,
          profile_type,
          name,
          endpoint,
          username,
          default_scope,
          notes,
          config_json,
          has_secret,
          created_at,
          updated_at
        FROM connection_profiles
        WHERE id = ?1
        "#,
    )?;

    let mut rows = statement.query(params![profile_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(ConnectionProfile {
            id: row.get(0)?,
            environment_id: row.get(1)?,
            profile_type: row.get(2)?,
            name: row.get(3)?,
            endpoint: row.get(4)?,
            username: row.get(5)?,
            default_scope: row.get(6)?,
            notes: row.get(7)?,
            config_json: row.get(8)?,
            has_secret: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        }));
    }

    Ok(None)
}

pub fn upsert_connection_profile(
    connection: &Connection,
    input: &UpsertConnectionProfileInput,
    profile_id: &str,
    has_secret: bool,
) -> Result<ConnectionProfile, rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let config_json = input
        .config_json
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "{}".to_string());

    connection.execute(
        r#"
        INSERT INTO connection_profiles (
          id,
          environment_id,
          profile_type,
          name,
          endpoint,
          username,
          default_scope,
          notes,
          config_json,
          has_secret,
          created_at,
          updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
        ON CONFLICT(id) DO UPDATE SET
          environment_id = excluded.environment_id,
          profile_type = excluded.profile_type,
          name = excluded.name,
          endpoint = excluded.endpoint,
          username = excluded.username,
          default_scope = excluded.default_scope,
          notes = excluded.notes,
          config_json = excluded.config_json,
          has_secret = excluded.has_secret,
          updated_at = excluded.updated_at
        "#,
        params![
            profile_id,
            input.environment_id,
            input.profile_type,
            input.name,
            input.endpoint,
            input.username,
            input.default_scope,
            input.notes,
            config_json,
            has_secret,
            now
        ],
    )?;

    Ok(ConnectionProfile {
        id: profile_id.to_string(),
        environment_id: input.environment_id.clone(),
        profile_type: input.profile_type.clone(),
        name: input.name.clone(),
        endpoint: input.endpoint.clone(),
        username: input.username.clone(),
        default_scope: input.default_scope.clone(),
        notes: input.notes.clone(),
        config_json,
        has_secret,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn update_connection_profile_secret_state(
    connection: &Connection,
    profile_id: &str,
    has_secret: bool,
) -> Result<(), rusqlite::Error> {
    connection.execute(
        "UPDATE connection_profiles SET has_secret = ?2, updated_at = ?3 WHERE id = ?1",
        params![profile_id, has_secret, Utc::now().to_rfc3339()],
    )?;

    Ok(())
}

pub fn validate_connection_profile(input: &UpsertConnectionProfileInput) -> ValidationResult {
    let mut messages = Vec::new();

    if input.name.trim().is_empty() {
        messages.push("Profile name is required.".to_string());
    }

    if input.environment_id.trim().is_empty() {
        messages.push("Environment is required.".to_string());
    }

    if input.profile_type.trim().is_empty() {
        messages.push("Profile type is required.".to_string());
    }

    if input.profile_type == "kubernetes" {
        let config_json = input.config_json.clone().unwrap_or_default();
        if input.endpoint.trim().is_empty() && !config_json.contains("kubeconfigPath") {
            messages.push(
                "Kubernetes profiles should provide an API endpoint or a kubeconfigPath in extra JSON."
                    .to_string(),
            );
        }
    }

    if matches!(input.profile_type.as_str(), "elk" | "nacos" | "redis" | "qwen")
        && input.endpoint.trim().is_empty()
    {
        messages.push("This profile type requires an endpoint or base URL.".to_string());
    }

    if input.profile_type == "ssh" && input.endpoint.trim().is_empty() {
        messages.push("SSH profiles require a host or host:port endpoint.".to_string());
    }

    if input.profile_type == "ssh" {
        if let Some(config_json) = &input.config_json {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(config_json) {
                if let Some(auth_mode) = config.get("authMode").and_then(serde_json::Value::as_str) {
                    if !matches!(auth_mode, "agent" | "key" | "password") {
                        messages.push("SSH authMode must be one of agent, key, or password.".to_string());
                    }
                }
                if let Some(private_key_path) = config.get("privateKeyPath").and_then(serde_json::Value::as_str) {
                    if private_key_path.trim().is_empty() {
                        messages.push("SSH privateKeyPath cannot be empty when provided.".to_string());
                    }
                }
            }
        }
    }

    if input.profile_type == "nacos" {
        if let Some(config_json) = &input.config_json {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(config_json) {
                if let Some(auth_mode) = config.get("authMode").and_then(serde_json::Value::as_str) {
                    if !matches!(auth_mode, "none" | "basic" | "bearer" | "accessToken") {
                        messages.push(
                            "Nacos authMode must be one of none, basic, bearer, or accessToken."
                                .to_string(),
                        );
                    }
                }
                if let Some(api_version) = config.get("apiVersion").and_then(serde_json::Value::as_str) {
                    if !matches!(api_version, "v1" | "v2") {
                        messages.push("Nacos apiVersion must be either v1 or v2.".to_string());
                    }
                }
            }
        }
    }

    if let Some(config_json) = &input.config_json {
        if !config_json.trim().is_empty() && serde_json::from_str::<serde_json::Value>(config_json).is_err() {
            messages.push("Extra JSON must be valid JSON.".to_string());
        }
    }

    if input
        .secret_value
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(false)
    {
        messages.push("Secret value cannot be empty when provided.".to_string());
    }

    if messages.is_empty() {
        messages.push("Profile metadata looks valid for local use.".to_string());
    }

    ValidationResult {
        ok: messages.len() == 1 && messages[0] == "Profile metadata looks valid for local use.",
        messages,
    }
}

pub fn create_chat_session(
    connection: &Connection,
    environment_id: &str,
    title: &str,
) -> Result<ChatSession, rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let session_id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        r#"
        INSERT INTO chat_sessions (id, environment_id, title, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, 'active', ?4, ?4)
        "#,
        params![session_id, environment_id, title, now],
    )?;

    Ok(ChatSession {
        id: session_id,
        environment_id: environment_id.to_string(),
        title: title.to_string(),
        status: "active".to_string(),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn get_chat_session(
    connection: &Connection,
    session_id: &str,
) -> Result<Option<ChatSession>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT id, environment_id, title, status, created_at, updated_at FROM chat_sessions WHERE id = ?1",
    )?;
    let mut rows = statement.query(params![session_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(ChatSession {
            id: row.get(0)?,
            environment_id: row.get(1)?,
            title: row.get(2)?,
            status: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        }));
    }

    Ok(None)
}

pub fn touch_chat_session(connection: &Connection, session_id: &str) -> Result<(), rusqlite::Error> {
    connection.execute(
        "UPDATE chat_sessions SET updated_at = ?2 WHERE id = ?1",
        params![session_id, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn list_chat_sessions(connection: &Connection) -> Result<Vec<ChatSession>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, environment_id, title, status, created_at, updated_at
        FROM chat_sessions
        ORDER BY updated_at DESC
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ChatSession {
            id: row.get(0)?,
            environment_id: row.get(1)?,
            title: row.get(2)?,
            status: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

pub fn append_chat_message(
    connection: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
    tool_name: Option<&str>,
    tool_call_id: Option<&str>,
) -> Result<ChatMessage, rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let message_id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        r#"
        INSERT INTO chat_messages (id, session_id, role, content, tool_name, tool_call_id, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![message_id, session_id, role, content, tool_name, tool_call_id, now],
    )?;

    Ok(ChatMessage {
        id: message_id,
        session_id: session_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tool_name: tool_name.map(str::to_string),
        tool_call_id: tool_call_id.map(str::to_string),
        created_at: now,
    })
}

pub fn list_chat_messages(
    connection: &Connection,
    session_id: &str,
) -> Result<Vec<ChatMessage>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, session_id, role, content, tool_name, tool_call_id, created_at
        FROM chat_messages
        WHERE session_id = ?1
        ORDER BY created_at ASC
        "#,
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        Ok(ChatMessage {
            id: row.get(0)?,
            session_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            tool_name: row.get(4)?,
            tool_call_id: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

pub fn insert_audit_log(
    connection: &Connection,
    session_id: Option<&str>,
    environment_id: Option<&str>,
    actor_type: &str,
    event_type: &str,
    tool_name: Option<&str>,
    target_ref: Option<&str>,
    request_json: Option<&str>,
    status: &str,
) -> Result<(), rusqlite::Error> {
    connection.execute(
        r#"
        INSERT INTO audit_logs (
          id, session_id, environment_id, actor_type, event_type, tool_name, target_ref, request_json, status, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        params![
            uuid::Uuid::new_v4().to_string(),
            session_id,
            environment_id,
            actor_type,
            event_type,
            tool_name,
            target_ref,
            request_json,
            status,
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn create_investigation(
    connection: &Connection,
    environment_id: &str,
    title: &str,
) -> Result<InvestigationSummary, rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let investigation_id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        r#"
        INSERT INTO investigations (id, title, environment_id, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, 'active', ?4, ?4)
        "#,
        params![investigation_id, title, environment_id, now],
    )?;

    Ok(InvestigationSummary {
        id: investigation_id,
        title: title.to_string(),
        environment_id: environment_id.to_string(),
        status: "active".to_string(),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn touch_investigation(connection: &Connection, investigation_id: &str) -> Result<(), rusqlite::Error> {
    connection.execute(
        "UPDATE investigations SET updated_at = ?2 WHERE id = ?1",
        params![investigation_id, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn get_investigation(
    connection: &Connection,
    investigation_id: &str,
) -> Result<Option<InvestigationSummary>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT id, title, environment_id, status, created_at, updated_at FROM investigations WHERE id = ?1",
    )?;
    let mut rows = statement.query(params![investigation_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(InvestigationSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            environment_id: row.get(2)?,
            status: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        }));
    }

    Ok(None)
}

pub fn list_investigations(connection: &Connection) -> Result<Vec<InvestigationSummary>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, title, environment_id, status, created_at, updated_at
        FROM investigations
        ORDER BY updated_at DESC
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(InvestigationSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            environment_id: row.get(2)?,
            status: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

pub fn add_investigation_evidence(
    connection: &Connection,
    investigation_id: &str,
    evidence_type: &str,
    title: &str,
    summary: &str,
    content_json: &str,
) -> Result<InvestigationEvidence, rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let evidence_id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        r#"
        INSERT INTO investigation_evidence (
          id, investigation_id, evidence_type, title, summary, content_json, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![evidence_id, investigation_id, evidence_type, title, summary, content_json, now],
    )?;

    Ok(InvestigationEvidence {
        id: evidence_id,
        investigation_id: investigation_id.to_string(),
        evidence_type: evidence_type.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        content_json: content_json.to_string(),
        created_at: now,
    })
}

pub fn list_investigation_evidence(
    connection: &Connection,
    investigation_id: &str,
) -> Result<Vec<InvestigationEvidence>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, investigation_id, evidence_type, title, summary, content_json, created_at
        FROM investigation_evidence
        WHERE investigation_id = ?1
        ORDER BY created_at DESC
        "#,
    )?;
    let rows = statement.query_map(params![investigation_id], |row| {
        Ok(InvestigationEvidence {
            id: row.get(0)?,
            investigation_id: row.get(1)?,
            evidence_type: row.get(2)?,
            title: row.get(3)?,
            summary: row.get(4)?,
            content_json: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

pub fn create_approval_request(
    connection: &Connection,
    environment_id: &str,
    action_type: &str,
    target_ref: &str,
    target_details_json: &str,
    risk_level: &str,
    rationale: &str,
    rollback_hint: &str,
) -> Result<ApprovalRequest, rusqlite::Error> {
    let now = Utc::now().to_rfc3339();
    let approval_id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        r#"
        INSERT INTO approval_requests (
          id, environment_id, action_type, target_ref, target_details_json, status, risk_level, rationale, rollback_hint, execution_summary, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?8, NULL, ?9, ?9)
        "#,
        params![
            approval_id,
            environment_id,
            action_type,
            target_ref,
            target_details_json,
            risk_level,
            rationale,
            rollback_hint,
            now
        ],
    )?;

    Ok(ApprovalRequest {
        id: approval_id,
        environment_id: environment_id.to_string(),
        action_type: action_type.to_string(),
        target_ref: target_ref.to_string(),
        status: "pending".to_string(),
        risk_level: risk_level.to_string(),
        rationale: rationale.to_string(),
        rollback_hint: rollback_hint.to_string(),
        execution_summary: None,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn list_approval_requests(connection: &Connection) -> Result<Vec<ApprovalRequest>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, environment_id, action_type, target_ref, status, risk_level, rationale, rollback_hint, execution_summary, created_at, updated_at
        FROM approval_requests
        ORDER BY updated_at DESC
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ApprovalRequest {
            id: row.get(0)?,
            environment_id: row.get(1)?,
            action_type: row.get(2)?,
            target_ref: row.get(3)?,
            status: row.get(4)?,
            risk_level: row.get(5)?,
            rationale: row.get(6)?,
            rollback_hint: row.get(7)?,
            execution_summary: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

pub fn get_approval_request(
    connection: &Connection,
    approval_id: &str,
) -> Result<Option<(ApprovalRequest, String)>, rusqlite::Error> {
    let mut statement = connection.prepare(
        r#"
        SELECT id, environment_id, action_type, target_ref, target_details_json, status, risk_level, rationale, rollback_hint, execution_summary, created_at, updated_at
        FROM approval_requests
        WHERE id = ?1
        "#,
    )?;
    let mut rows = statement.query(params![approval_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some((
            ApprovalRequest {
                id: row.get(0)?,
                environment_id: row.get(1)?,
                action_type: row.get(2)?,
                target_ref: row.get(3)?,
                status: row.get(5)?,
                risk_level: row.get(6)?,
                rationale: row.get(7)?,
                rollback_hint: row.get(8)?,
                execution_summary: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            },
            row.get(4)?,
        )));
    }
    Ok(None)
}

pub fn update_approval_status(
    connection: &Connection,
    approval_id: &str,
    status: &str,
    execution_summary: Option<&str>,
) -> Result<(), rusqlite::Error> {
    connection.execute(
        "UPDATE approval_requests SET status = ?2, execution_summary = COALESCE(?3, execution_summary), updated_at = ?4 WHERE id = ?1",
        params![approval_id, status, execution_summary, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}
