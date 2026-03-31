use tauri::State;

use crate::models::app_health::AppHealth;
use crate::models::environment::EnvironmentProfile;
use crate::AppState;

#[tauri::command]
pub fn get_app_health(state: State<'_, AppState>) -> AppHealth {
    AppHealth {
        app_name: "ADBCHelper".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database_ready: state.database_ready,
        storage_path: state.storage_path.clone(),
    }
}

#[tauri::command]
pub fn list_environments(state: State<'_, AppState>) -> Result<Vec<EnvironmentProfile>, String> {
    let connection = rusqlite::Connection::open(&state.storage_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
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
        )
        .map_err(|error| error.to_string())?;

    let environment_rows = statement
        .query_map([], |row| {
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
        })
        .map_err(|error| error.to_string())?;

    let environments = environment_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    Ok(environments)
}
