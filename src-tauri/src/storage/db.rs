use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection};
use thiserror::Error;

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
