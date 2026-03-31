use std::path::Path;
use std::process::Command;
use std::time::Duration;

use rusqlite::Connection;
use serde_json::Value;
use tauri::State;

use crate::hardening::{run_command_with_timeout, sanitize_and_mask_text};
use crate::models::approval::{ApprovalRequest, CreateApprovalInput, ExecuteApprovalInput};
use crate::storage::db;
use crate::AppState;

fn open_connection(storage_path: &str) -> Result<Connection, String> {
    Connection::open(storage_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_approval_requests(state: State<'_, AppState>) -> Result<Vec<ApprovalRequest>, String> {
    let connection = open_connection(&state.storage_path)?;
    db::list_approval_requests(&connection).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_approval_request(
    state: State<'_, AppState>,
    input: CreateApprovalInput,
) -> Result<ApprovalRequest, String> {
    let connection = open_connection(&state.storage_path)?;
    let risk_level = classify_risk(&input.environment_id, &input.action_type);
    validate_production_safety(&input.environment_id, &input.action_type, &input.rationale, &input.target_details_json)?;
    let rollback_hint = input
        .rollback_hint
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_rollback_hint(&input.action_type));

    let approval = db::create_approval_request(
        &connection,
        &input.environment_id,
        &input.action_type,
        &input.target_ref,
        &input.target_details_json,
        &risk_level,
        &input.rationale,
        &rollback_hint,
    )
    .map_err(|error| error.to_string())?;

    db::insert_audit_log(
        &connection,
        None,
        Some(&approval.environment_id),
        "user",
        "approval_request_create",
        Some(&approval.action_type),
        Some(&approval.target_ref),
        Some(&approval.rationale),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    Ok(approval)
}

#[tauri::command]
pub fn approve_request(
    state: State<'_, AppState>,
    approval_id: String,
) -> Result<ApprovalRequest, String> {
    let connection = open_connection(&state.storage_path)?;
    let (approval, _) = db::get_approval_request(&connection, &approval_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Selected approval request no longer exists.".to_string())?;

    if approval.status != "pending" {
        return Err("Only pending requests can be approved.".to_string());
    }

    db::update_approval_status(&connection, &approval_id, "approved", None)
        .map_err(|error| error.to_string())?;
    db::insert_audit_log(
        &connection,
        None,
        Some(&approval.environment_id),
        "user",
        "approval_request_approve",
        Some(&approval.action_type),
        Some(&approval.target_ref),
        Some(&approval.id),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    db::list_approval_requests(&connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|item| item.id == approval_id)
        .ok_or_else(|| "Approved request could not be reloaded.".to_string())
}

#[tauri::command]
pub fn execute_approval_request(
    state: State<'_, AppState>,
    input: ExecuteApprovalInput,
) -> Result<ApprovalRequest, String> {
    let connection = open_connection(&state.storage_path)?;
    let (approval, target_details_json) = db::get_approval_request(&connection, &input.approval_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Selected approval request no longer exists.".to_string())?;

    if approval.status != "approved" {
        return Err("Only approved requests can be executed.".to_string());
    }

    let target_details = serde_json::from_str::<Value>(&target_details_json)
        .map_err(|error| format!("Invalid approval target details: {error}"))?;
    let execution_summary = execute_action(&connection, &approval, &target_details)?;

    db::update_approval_status(&connection, &approval.id, "executed", Some(&execution_summary))
        .map_err(|error| error.to_string())?;
    db::insert_audit_log(
        &connection,
        None,
        Some(&approval.environment_id),
        "user",
        "approval_request_execute",
        Some(&approval.action_type),
        Some(&approval.target_ref),
        Some(&execution_summary),
        "completed",
    )
    .map_err(|error| error.to_string())?;

    db::list_approval_requests(&connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|item| item.id == approval.id)
        .ok_or_else(|| "Executed request could not be reloaded.".to_string())
}

fn classify_risk(environment_id: &str, action_type: &str) -> String {
    match (environment_id, action_type) {
        ("prod", "restart_pod") => "high".to_string(),
        ("prod", "scale_deployment") => "critical".to_string(),
        ("prod", "reload_nginx") => "high".to_string(),
        (_, "scale_deployment") => "high".to_string(),
        (_, "reload_nginx") => "medium".to_string(),
        _ => "medium".to_string(),
    }
}

fn default_rollback_hint(action_type: &str) -> String {
    match action_type {
        "restart_pod" => "Kubernetes controller should recreate the pod; verify readiness and redeploy if restart loops persist.".to_string(),
        "scale_deployment" => "Scale the deployment back to the previous replica count if traffic or error rate worsens.".to_string(),
        "reload_nginx" => "Restore the previous Nginx configuration and reload again if upstream routing regresses.".to_string(),
        _ => "Revert the action through the same controlled approval flow if the outcome is unsafe.".to_string(),
    }
}

fn execute_action(
    connection: &Connection,
    approval: &ApprovalRequest,
    target_details: &Value,
) -> Result<String, String> {
    match approval.action_type.as_str() {
        "restart_pod" => execute_restart_pod(connection, approval, target_details),
        "scale_deployment" => execute_scale_deployment(connection, approval, target_details),
        "reload_nginx" => execute_reload_nginx(connection, approval, target_details),
        other => Err(format!("Unsupported approval action: {other}")),
    }
}

fn execute_restart_pod(
    connection: &Connection,
    approval: &ApprovalRequest,
    target_details: &Value,
) -> Result<String, String> {
    let profile = resolve_profile(connection, &approval.environment_id, "kubernetes")?;
    let namespace = required_string(target_details, "namespace")?;
    let pod = required_string(target_details, "podName")?;
    let config = KubernetesExecConfig::from_profile(&profile)?;

    let mut command = Command::new("kubectl");
    config.apply(&mut command)?;
    command.arg("delete").arg("pod").arg(&pod).arg("-n").arg(&namespace);
    let output = run_command_with_timeout(&mut command, Duration::from_secs(10), "kubectl delete pod")?;
    if !output.status.success() {
        return Err(command_error("kubectl delete pod", &output));
    }

    Ok(format!(
        "Restarted pod {} in namespace {}. {}",
        pod,
        namespace,
        sanitize_and_mask_text(String::from_utf8_lossy(&output.stdout).trim())
    ))
}

fn execute_scale_deployment(
    connection: &Connection,
    approval: &ApprovalRequest,
    target_details: &Value,
) -> Result<String, String> {
    let profile = resolve_profile(connection, &approval.environment_id, "kubernetes")?;
    let namespace = required_string(target_details, "namespace")?;
    let deployment = required_string(target_details, "deploymentName")?;
    let replicas = target_details
        .get("replicas")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Approval target is missing replicas.".to_string())?;
    let config = KubernetesExecConfig::from_profile(&profile)?;

    let mut command = Command::new("kubectl");
    config.apply(&mut command)?;
    command
        .arg("scale")
        .arg("deployment")
        .arg(&deployment)
        .arg("-n")
        .arg(&namespace)
        .arg(format!("--replicas={replicas}"));
    let output = run_command_with_timeout(&mut command, Duration::from_secs(10), "kubectl scale deployment")?;
    if !output.status.success() {
        return Err(command_error("kubectl scale deployment", &output));
    }

    Ok(format!(
        "Scaled deployment {} in namespace {} to {} replicas. {}",
        deployment,
        namespace,
        replicas,
        sanitize_and_mask_text(String::from_utf8_lossy(&output.stdout).trim())
    ))
}

fn execute_reload_nginx(
    connection: &Connection,
    approval: &ApprovalRequest,
    target_details: &Value,
) -> Result<String, String> {
    let profile = resolve_profile(connection, &approval.environment_id, "ssh")?;
    let ssh = SshExecConfig::from_profile(&profile, target_details.get("host").and_then(Value::as_str))?;

    let mut command = Command::new("ssh");
    ssh.apply(&mut command)?;
    command.arg(ssh.render_target());
    command.arg("nginx -s reload || systemctl reload nginx || sudo systemctl reload nginx");
    let output = run_command_with_timeout(&mut command, Duration::from_secs(10), "ssh reload nginx")?;
    if !output.status.success() {
        return Err(command_error("ssh reload nginx", &output));
    }

    Ok(format!(
        "Reloaded Nginx on {}. {}",
        ssh.render_target(),
        sanitize_and_mask_text(String::from_utf8_lossy(&output.stdout).trim())
    ))
}

fn resolve_profile(
    connection: &Connection,
    environment_id: &str,
    profile_type: &str,
) -> Result<crate::models::connection_profile::ConnectionProfile, String> {
    db::list_connection_profiles(connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|profile| profile.environment_id == environment_id && profile.profile_type == profile_type)
        .ok_or_else(|| format!("No {} profile found for environment {}.", profile_type, environment_id))
}

fn required_string(target_details: &Value, key: &str) -> Result<String, String> {
    target_details
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("Approval target is missing {}.", key))
}

fn command_error(prefix: &str, output: &std::process::Output) -> String {
    let stderr = sanitize_and_mask_text(String::from_utf8_lossy(&output.stderr).trim());
    if stderr.is_empty() {
        format!("{} failed with status {}.", prefix, output.status)
    } else {
        format!("{} failed: {}", prefix, stderr)
    }
}

struct KubernetesExecConfig {
    kubeconfig_path: Option<String>,
    context: Option<String>,
}

impl KubernetesExecConfig {
    fn from_profile(profile: &crate::models::connection_profile::ConnectionProfile) -> Result<Self, String> {
        let config = serde_json::from_str::<Value>(&profile.config_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        Ok(Self {
            kubeconfig_path: config.get("kubeconfigPath").and_then(Value::as_str).map(str::to_string),
            context: config.get("context").and_then(Value::as_str).map(str::to_string),
        })
    }

    fn apply(&self, command: &mut Command) -> Result<(), String> {
        if let Some(kubeconfig_path) = &self.kubeconfig_path {
            if !Path::new(kubeconfig_path).exists() {
                return Err(format!("Configured kubeconfigPath does not exist: {kubeconfig_path}"));
            }
            command.arg("--kubeconfig").arg(kubeconfig_path);
        }
        if let Some(context) = &self.context {
            command.arg("--context").arg(context);
        }
        Ok(())
    }
}

struct SshExecConfig {
    host: String,
    port: Option<u16>,
    username: Option<String>,
    private_key_path: Option<String>,
}

impl SshExecConfig {
    fn from_profile(
        profile: &crate::models::connection_profile::ConnectionProfile,
        host_override: Option<&str>,
    ) -> Result<Self, String> {
        let config = serde_json::from_str::<Value>(&profile.config_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        let endpoint = host_override.unwrap_or(profile.endpoint.as_str()).trim();
        if endpoint.is_empty() {
            return Err("SSH approval action requires a target host.".to_string());
        }
        let (host, port) = parse_host_and_port(endpoint)?;
        Ok(Self {
            host,
            port: config
                .get("port")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .or(port),
            username: profile.username.clone().filter(|value| !value.trim().is_empty()),
            private_key_path: config
                .get("privateKeyPath")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    fn apply(&self, command: &mut Command) -> Result<(), String> {
        command.arg("-o").arg("BatchMode=yes");
        command.arg("-o").arg("ConnectTimeout=5");
        if let Some(port) = self.port {
            command.arg("-p").arg(port.to_string());
        }
        if let Some(private_key_path) = &self.private_key_path {
            if !Path::new(private_key_path).exists() {
                return Err(format!("Configured privateKeyPath does not exist: {private_key_path}"));
            }
            command.arg("-i").arg(private_key_path);
        }
        Ok(())
    }

    fn render_target(&self) -> String {
        match &self.username {
            Some(username) => format!("{username}@{}", self.host),
            None => self.host.clone(),
        }
    }
}

fn parse_host_and_port(endpoint: &str) -> Result<(String, Option<u16>), String> {
    if let Some((host, port)) = endpoint.rsplit_once(':') {
        if !host.contains(':') {
            if let Ok(parsed) = port.parse::<u16>() {
                return Ok((host.to_string(), Some(parsed)));
            }
        }
    }
    Ok((endpoint.to_string(), None))
}

fn validate_production_safety(
    environment_id: &str,
    action_type: &str,
    rationale: &str,
    target_details_json: &str,
) -> Result<(), String> {
    if environment_id != "prod" {
        return Ok(());
    }

    if rationale.trim().len() < 12 {
        return Err("Production approval requests require a more specific rationale.".to_string());
    }

    let details = serde_json::from_str::<Value>(target_details_json)
        .map_err(|error| format!("Invalid approval target details: {error}"))?;

    if action_type == "scale_deployment" {
        let replicas = details
            .get("replicas")
            .and_then(Value::as_u64)
            .ok_or_else(|| "Production scale requests must include a replica count.".to_string())?;
        if replicas == 0 {
            return Err("Scaling a production deployment to 0 is blocked by safety policy.".to_string());
        }
        if replicas > 20 {
            return Err("Scaling a production deployment above 20 replicas is blocked by safety policy.".to_string());
        }
    }

    Ok(())
}
