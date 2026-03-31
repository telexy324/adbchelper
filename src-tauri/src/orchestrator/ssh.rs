use chrono::{Duration, Utc};
use rusqlite::Connection;

use crate::models::connection_profile::ConnectionProfile;
use crate::models::ssh::{
    SshDiagnosticsInput, SshDiagnosticsResponse, SshHealthMetric, SshLogLine,
};
use crate::storage::db;

const COMMAND_PRESETS: [(&str, &str); 4] = [
    ("system_overview", "uptime && df -h && free -m"),
    ("check_process_ports", "ps aux | grep -E 'nginx|java|node' && ss -ltn"),
    ("tail_app_log", "tail -n 80 /var/log/app/application.log"),
    ("tail_nginx_error", "tail -n 80 /var/log/nginx/error.log"),
];

pub fn run_diagnostics(
    connection: &Connection,
    input: SshDiagnosticsInput,
) -> Result<SshDiagnosticsResponse, String> {
    let profile = resolve_ssh_profile(connection, &input.environment_id)?;
    let target_host = input
        .host
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| profile.endpoint.clone());
    let executed_command = resolve_command(&input.command_preset, input.log_path.as_deref())?;
    let health_summary = sample_health_metrics(&input.environment_id, &target_host);
    let log_lines = sample_log_lines(&input.command_preset, &target_host);
    let recommended_actions = build_recommendations(&input.command_preset, &health_summary);
    let summary_headline = build_summary(&target_host, &input.command_preset, &health_summary, &log_lines);

    Ok(SshDiagnosticsResponse {
        environment_id: input.environment_id,
        adapter_mode: format!("mock-ssh-adapter ({})", profile.name),
        target_host,
        command_preset: input.command_preset,
        executed_command,
        allowed_commands: allowed_commands(),
        health_summary,
        log_lines,
        summary_headline,
        recommended_actions,
    })
}

fn resolve_ssh_profile(connection: &Connection, environment_id: &str) -> Result<ConnectionProfile, String> {
    db::list_connection_profiles(connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|profile| profile.environment_id == environment_id && profile.profile_type == "ssh")
        .ok_or_else(|| "No SSH profile found for this environment. Add one in Settings first.".to_string())
}

fn resolve_command(command_preset: &str, log_path: Option<&str>) -> Result<String, String> {
    match command_preset {
        "tail_app_log" => Ok(format!(
            "tail -n 80 {}",
            log_path
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("/var/log/app/application.log")
        )),
        "tail_nginx_error" => Ok(format!(
            "tail -n 80 {}",
            log_path
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("/var/log/nginx/error.log")
        )),
        other => COMMAND_PRESETS
            .iter()
            .find(|(name, _)| *name == other)
            .map(|(_, command)| (*command).to_string())
            .ok_or_else(|| format!("Unsupported SSH command preset: {other}")),
    }
}

fn allowed_commands() -> Vec<String> {
    COMMAND_PRESETS
        .iter()
        .map(|(_, command)| (*command).to_string())
        .collect()
}

fn sample_health_metrics(environment_id: &str, target_host: &str) -> Vec<SshHealthMetric> {
    let is_prod = environment_id == "prod";
    vec![
        SshHealthMetric {
            label: "CPU".to_string(),
            status: if is_prod { "warning" } else { "healthy" }.to_string(),
            value: if is_prod { "78%" } else { "34%" }.to_string(),
            detail: format!("Load average on {target_host} is within watch range."),
        },
        SshHealthMetric {
            label: "Memory".to_string(),
            status: if is_prod { "warning" } else { "healthy" }.to_string(),
            value: if is_prod { "81%" } else { "46%" }.to_string(),
            detail: "Heap and page cache are elevated but not exhausted.".to_string(),
        },
        SshHealthMetric {
            label: "Disk".to_string(),
            status: "healthy".to_string(),
            value: if is_prod { "68%" } else { "52%" }.to_string(),
            detail: "/var still has enough headroom for log growth.".to_string(),
        },
        SshHealthMetric {
            label: "Ports".to_string(),
            status: if is_prod { "warning" } else { "healthy" }.to_string(),
            value: if is_prod { "8080, 8443, 9100" } else { "8080, 9100" }.to_string(),
            detail: "Expected listeners are present and the process table matches the host role.".to_string(),
        },
    ]
}

fn sample_log_lines(command_preset: &str, target_host: &str) -> Vec<SshLogLine> {
    let now = Utc::now();
    match command_preset {
        "tail_nginx_error" => vec![
            log_line(now - Duration::minutes(8), "nginx/error.log", "ERROR", &format!(
                "{target_host} upstream prematurely closed connection while reading response header from upstream"
            )),
            log_line(now - Duration::minutes(6), "nginx/error.log", "ERROR", &format!(
                "{target_host} connect() failed (111: Connection refused) while connecting to upstream"
            )),
            log_line(now - Duration::minutes(4), "nginx/error.log", "WARN", &format!(
                "{target_host} cached SSL session removed due to upstream instability"
            )),
        ],
        "check_process_ports" => vec![
            log_line(now - Duration::minutes(7), "process-audit", "INFO", "java payment-service pid=28491 listening on 8080"),
            log_line(now - Duration::minutes(6), "process-audit", "INFO", "node metrics-sidecar pid=1221 listening on 9100"),
            log_line(now - Duration::minutes(5), "process-audit", "WARN", "nginx workers restarted once in the last 10 minutes"),
        ],
        _ => vec![
            log_line(now - Duration::minutes(9), "application.log", "ERROR", "Database pool timeout while serving checkout request 5512"),
            log_line(now - Duration::minutes(6), "application.log", "WARN", "Retrying downstream inventory call after socket timeout"),
            log_line(now - Duration::minutes(3), "application.log", "ERROR", "Redis reconnect attempt took longer than 1200ms"),
        ],
    }
}

fn build_recommendations(command_preset: &str, health_summary: &[SshHealthMetric]) -> Vec<String> {
    let mut actions = vec![
        "Compare this host snapshot with Kubernetes pod health and recent log clusters in the same time window.".to_string(),
        "Keep remediation read-only for now and capture the strongest evidence into the investigation timeline.".to_string(),
    ];

    if health_summary.iter().any(|metric| metric.status == "warning") {
        actions.push("Check whether elevated CPU or memory aligns with the first error spike in application logs.".to_string());
    }

    if command_preset == "tail_nginx_error" {
        actions.push("Correlate Nginx upstream failures with application listener health and recent deploy activity.".to_string());
    }

    actions
}

fn build_summary(
    target_host: &str,
    command_preset: &str,
    health_summary: &[SshHealthMetric],
    log_lines: &[SshLogLine],
) -> String {
    let warning_count = health_summary.iter().filter(|metric| metric.status == "warning").count();
    let top_log = log_lines
        .first()
        .map(|line| line.message.clone())
        .unwrap_or_else(|| "No log samples returned.".to_string());

    format!(
        "{target_host} returned {warning_count} warning metric(s) for {command_preset}. Most recent signal: {top_log}"
    )
}

fn log_line(timestamp: chrono::DateTime<Utc>, source: &str, level: &str, message: &str) -> SshLogLine {
    SshLogLine {
        timestamp: timestamp.to_rfc3339(),
        source: source.to_string(),
        level: level.to_string(),
        message: message.to_string(),
    }
}
