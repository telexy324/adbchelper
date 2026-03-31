use std::path::Path;
use std::process::Command;

use chrono::Utc;
use rusqlite::Connection;
use serde_json::Value;

use crate::models::connection_profile::ConnectionProfile;
use crate::models::ssh::{
    SshDiagnosticsInput, SshDiagnosticsResponse, SshHealthMetric, SshLogLine,
};
use crate::storage::{db, secrets};

const COMMAND_PRESETS: [(&str, &str); 4] = [
    (
        "system_overview",
        "printf '__LOAD__\n'; uptime; printf '\n__MEM__\n'; free -m; printf '\n__DISK__\n'; df -h / /var 2>/dev/null || df -h; printf '\n__END__\n'",
    ),
    (
        "check_process_ports",
        "printf '__PROCESSES__\n'; ps aux | grep -E 'nginx|java|node' | grep -v grep | head -n 20; printf '\n__PORTS__\n'; ss -ltn | head -n 20; printf '\n__END__\n'",
    ),
    ("tail_app_log", "tail -n 80 /var/log/app/application.log"),
    ("tail_nginx_error", "tail -n 80 /var/log/nginx/error.log"),
];

pub fn run_diagnostics(
    connection: &Connection,
    input: SshDiagnosticsInput,
) -> Result<SshDiagnosticsResponse, String> {
    let profile = resolve_ssh_profile(connection, &input.environment_id)?;
    let ssh_config = SshConfig::from_profile(&profile, input.host.as_deref())?;
    let secret = if profile.has_secret {
        Some(secrets::get_profile_secret(&profile.id).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let remote_command = resolve_command(&input.command_preset, input.log_path.as_deref())?;
    let execution = execute_ssh_command(&ssh_config, &remote_command, secret.as_deref())?;
    let target_host = ssh_config.render_target();
    let health_summary = parse_health_summary(&input.command_preset, &target_host, &execution.stdout);
    let log_lines = parse_log_lines(&input.command_preset, &execution.stdout);
    let recommended_actions = build_recommendations(&input.command_preset, &health_summary);
    let summary_headline = build_summary(&target_host, &input.command_preset, &health_summary, &log_lines);

    Ok(SshDiagnosticsResponse {
        environment_id: input.environment_id,
        adapter_mode: format!("ssh-cli-profile ({})", profile.name),
        target_host,
        command_preset: input.command_preset,
        executed_command: remote_command,
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
            sanitize_remote_log_path(
                log_path
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("/var/log/app/application.log")
            )?
        )),
        "tail_nginx_error" => Ok(format!(
            "tail -n 80 {}",
            sanitize_remote_log_path(
                log_path
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("/var/log/nginx/error.log")
            )?
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

fn parse_health_summary(command_preset: &str, target_host: &str, stdout: &str) -> Vec<SshHealthMetric> {
    match command_preset {
        "check_process_ports" => parse_process_metrics(stdout),
        "tail_app_log" | "tail_nginx_error" => parse_log_health_metrics(stdout),
        _ => parse_system_metrics(target_host, stdout),
    }
}

fn parse_log_lines(command_preset: &str, stdout: &str) -> Vec<SshLogLine> {
    let now = Utc::now();
    let source = match command_preset {
        "tail_nginx_error" => "nginx/error.log",
        "check_process_ports" => "process-audit",
        "system_overview" => "host-metrics",
        _ => "application.log",
    };

    stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let level = detect_level(trimmed);
            Some(log_line(now, source, &level, trimmed))
        })
        .take(12)
        .collect()
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

fn parse_system_metrics(target_host: &str, stdout: &str) -> Vec<SshHealthMetric> {
    let load_value = extract_after_marker(stdout, "__LOAD__", "__MEM__")
        .and_then(|section| section.lines().next().map(str::trim).map(str::to_string))
        .unwrap_or_else(|| "unavailable".to_string());
    let memory_value = extract_after_marker(stdout, "__MEM__", "__DISK__")
        .and_then(find_mem_value)
        .unwrap_or_else(|| "unavailable".to_string());
    let disk_value = extract_after_marker(stdout, "__DISK__", "__END__")
        .and_then(find_disk_value)
        .unwrap_or_else(|| "unavailable".to_string());

    vec![
        SshHealthMetric {
            label: "CPU".to_string(),
            status: infer_status(&load_value),
            value: load_value,
            detail: format!("Load sample collected from {target_host}."),
        },
        SshHealthMetric {
            label: "Memory".to_string(),
            status: infer_status(&memory_value),
            value: memory_value,
            detail: "Memory summary parsed from remote host output.".to_string(),
        },
        SshHealthMetric {
            label: "Disk".to_string(),
            status: infer_status(&disk_value),
            value: disk_value,
            detail: "/ and /var usage returned by the host.".to_string(),
        },
        SshHealthMetric {
            label: "Ports".to_string(),
            status: "healthy".to_string(),
            value: "n/a".to_string(),
            detail: "Use the process and ports preset for listener-level diagnostics.".to_string(),
        },
    ]
}

fn parse_process_metrics(stdout: &str) -> Vec<SshHealthMetric> {
    let process_section = extract_after_marker(stdout, "__PROCESSES__", "__PORTS__").unwrap_or_default();
    let port_section = extract_after_marker(stdout, "__PORTS__", "__END__").unwrap_or_default();
    let process_count = process_section
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let listeners = port_section
        .lines()
        .filter(|line| line.contains("LISTEN"))
        .map(|line| line.split_whitespace().last().unwrap_or_default().to_string())
        .take(5)
        .collect::<Vec<_>>();

    vec![
        SshHealthMetric {
            label: "CPU".to_string(),
            status: "healthy".to_string(),
            value: "n/a".to_string(),
            detail: "Use system overview for CPU metrics.".to_string(),
        },
        SshHealthMetric {
            label: "Memory".to_string(),
            status: "healthy".to_string(),
            value: "n/a".to_string(),
            detail: "Use system overview for memory metrics.".to_string(),
        },
        SshHealthMetric {
            label: "Disk".to_string(),
            status: "healthy".to_string(),
            value: "n/a".to_string(),
            detail: "Use system overview for disk metrics.".to_string(),
        },
        SshHealthMetric {
            label: "Ports".to_string(),
            status: if listeners.is_empty() { "warning" } else { "healthy" }.to_string(),
            value: if listeners.is_empty() {
                "no listeners found".to_string()
            } else {
                listeners.join(", ")
            },
            detail: format!("{process_count} matching process rows returned from the host."),
        },
    ]
}

fn parse_log_health_metrics(stdout: &str) -> Vec<SshHealthMetric> {
    let error_count = stdout.lines().filter(|line| detect_level(line) == "ERROR").count();
    let warn_count = stdout.lines().filter(|line| detect_level(line) == "WARN").count();

    vec![
        SshHealthMetric {
            label: "CPU".to_string(),
            status: "healthy".to_string(),
            value: "n/a".to_string(),
            detail: "Tail commands focus on log evidence rather than host load.".to_string(),
        },
        SshHealthMetric {
            label: "Memory".to_string(),
            status: "healthy".to_string(),
            value: "n/a".to_string(),
            detail: "Tail commands focus on log evidence rather than memory pressure.".to_string(),
        },
        SshHealthMetric {
            label: "Disk".to_string(),
            status: "healthy".to_string(),
            value: "n/a".to_string(),
            detail: "Disk usage is not sampled by tail-only commands.".to_string(),
        },
        SshHealthMetric {
            label: "Ports".to_string(),
            status: if error_count > 0 { "warning" } else { "healthy" }.to_string(),
            value: format!("{error_count} errors / {warn_count} warnings"),
            detail: "Derived from the current server log sample.".to_string(),
        },
    ]
}

fn extract_after_marker<'a>(stdout: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let (_, after_start) = stdout.split_once(start)?;
    let section = after_start.trim_start_matches('\n');
    if end == "__END__" {
        return Some(section.trim());
    }
    let (body, _) = section.split_once(end)?;
    Some(body.trim())
}

fn find_mem_value(section: &str) -> Option<String> {
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Mem:") {
            let parts = trimmed.split_whitespace().collect::<Vec<_>>();
            if parts.len() >= 3 {
                return Some(format!("{} used of {} MB", parts[2], parts[1]));
            }
        }
    }
    None
}

fn find_disk_value(section: &str) -> Option<String> {
    section
        .lines()
        .find(|line| line.contains(" /") || line.contains(" /var"))
        .map(|line| {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.len() >= 5 {
                parts[4].to_string()
            } else {
                line.trim().to_string()
            }
        })
}

fn infer_status(value: &str) -> String {
    if value.contains("8") || value.contains("9") {
        "warning".to_string()
    } else {
        "healthy".to_string()
    }
}

fn detect_level(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("failed") || lower.contains("refused") {
        "ERROR".to_string()
    } else if lower.contains("warn") || lower.contains("timeout") {
        "WARN".to_string()
    } else {
        "INFO".to_string()
    }
}

fn sanitize_remote_log_path(path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("SSH log path cannot be empty.".to_string());
    }
    if !trimmed.starts_with('/') {
        return Err("SSH log path must be an absolute path.".to_string());
    }
    if trimmed
        .chars()
        .all(|char| char.is_ascii_alphanumeric() || matches!(char, '/' | '.' | '_' | '-'))
    {
        Ok(trimmed.to_string())
    } else {
        Err("SSH log path contains unsupported characters.".to_string())
    }
}

fn log_line(timestamp: chrono::DateTime<Utc>, source: &str, level: &str, message: &str) -> SshLogLine {
    SshLogLine {
        timestamp: timestamp.to_rfc3339(),
        source: source.to_string(),
        level: level.to_string(),
        message: message.to_string(),
    }
}

#[derive(Debug, Clone)]
struct SshConfig {
    host: String,
    port: Option<u16>,
    username: Option<String>,
    private_key_path: Option<String>,
    auth_mode: String,
    strict_host_key_checking: bool,
    known_hosts_path: Option<String>,
}

impl SshConfig {
    fn from_profile(profile: &ConnectionProfile, host_override: Option<&str>) -> Result<Self, String> {
        let config_json = profile
            .config_json
            .trim()
            .is_empty()
            .then(|| "{}".to_string())
            .unwrap_or_else(|| profile.config_json.clone());
        let config = serde_json::from_str::<Value>(&config_json)
            .map_err(|error| format!("Invalid SSH profile JSON: {error}"))?;

        let endpoint = host_override
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(profile.endpoint.trim());
        if endpoint.is_empty() {
            return Err("SSH profile is missing a host or endpoint.".to_string());
        }

        let (host, endpoint_port) = parse_host_and_port(endpoint)?;
        let port = config
            .get("port")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .or(endpoint_port);
        let private_key_path = config
            .get("privateKeyPath")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let auth_mode = config
            .get("authMode")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(if private_key_path.is_some() { "key" } else { "agent" })
            .to_string();
        let known_hosts_path = config
            .get("knownHostsPath")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let strict_host_key_checking = config
            .get("strictHostKeyChecking")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        Ok(Self {
            host,
            port,
            username: profile.username.clone().filter(|value| !value.trim().is_empty()),
            private_key_path,
            auth_mode,
            strict_host_key_checking,
            known_hosts_path,
        })
    }

    fn render_target(&self) -> String {
        match &self.username {
            Some(username) => format!("{username}@{}", self.host),
            None => self.host.clone(),
        }
    }
}

struct SshExecution {
    stdout: String,
}

fn execute_ssh_command(
    config: &SshConfig,
    remote_command: &str,
    secret: Option<&str>,
) -> Result<SshExecution, String> {
    if !is_valid_remote_command(remote_command) {
        return Err("Remote command is not in the approved whitelist.".to_string());
    }
    if config.auth_mode == "password" {
        return Err(
            "SSH profiles using password auth are not supported yet. Configure authMode=agent or authMode=key in Extra JSON."
                .to_string(),
        );
    }

    let ssh_binary = "ssh";
    let mut command = Command::new(ssh_binary);
    command.arg("-o").arg("BatchMode=yes");
    command.arg("-o").arg("ConnectTimeout=5");
    if config.strict_host_key_checking {
        command.arg("-o").arg("StrictHostKeyChecking=yes");
    } else {
        command.arg("-o").arg("StrictHostKeyChecking=no");
        command.arg("-o").arg("UserKnownHostsFile=/dev/null");
    }
    if let Some(known_hosts_path) = &config.known_hosts_path {
        command.arg("-o").arg(format!("UserKnownHostsFile={known_hosts_path}"));
    }
    if let Some(port) = config.port {
        command.arg("-p").arg(port.to_string());
    }
    if let Some(private_key_path) = &config.private_key_path {
        if !Path::new(private_key_path).exists() {
            return Err(format!("Configured privateKeyPath does not exist: {private_key_path}"));
        }
        command.arg("-i").arg(private_key_path);
    }
    if let Some(passphrase) = secret {
        if !passphrase.trim().is_empty() {
            command.arg("-o").arg("PreferredAuthentications=publickey");
        }
    }

    command.arg(config.render_target());
    command.arg(remote_command);

    let output = command
        .output()
        .map_err(|error| format!("Failed to execute local ssh client: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("SSH command failed with status {}.", output.status)
        } else {
            format!("SSH command failed: {stderr}")
        });
    }

    Ok(SshExecution {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
    })
}

fn is_valid_remote_command(command: &str) -> bool {
    allowed_commands()
        .into_iter()
        .any(|allowed| command == allowed || (allowed.starts_with("tail -n 80 ") && command.starts_with("tail -n 80 ")))
}

fn parse_host_and_port(endpoint: &str) -> Result<(String, Option<u16>), String> {
    if endpoint.contains(' ') {
        return Err("SSH host cannot contain spaces.".to_string());
    }

    if let Some((host, port)) = endpoint.rsplit_once(':') {
        if host.contains(':') {
            return Ok((endpoint.to_string(), None));
        }
        if let Ok(port) = port.parse::<u16>() {
            return Ok((host.to_string(), Some(port)));
        }
    }

    Ok((endpoint.to_string(), None))
}
