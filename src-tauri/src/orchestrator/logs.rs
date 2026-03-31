use std::collections::{BTreeSet, HashMap};

use chrono::{Duration, Utc};
use rusqlite::Connection;
use uuid::Uuid;

use crate::models::connection_profile::ConnectionProfile;
use crate::models::logs::{LogCluster, LogEntry, LogSearchInput, LogSearchResponse, LogSummary};
use crate::storage::db;

pub fn search_logs(
    connection: &Connection,
    input: LogSearchInput,
) -> Result<LogSearchResponse, String> {
    let adapter_mode = resolve_adapter_mode(connection, &input.environment_id)?;
    let query = build_query_string(&input);
    let entries = filter_entries(sample_entries(), &input);
    let clusters = cluster_entries(&entries);
    let summary = build_summary(&input, &entries, &clusters);

    Ok(LogSearchResponse {
        environment_id: input.environment_id,
        time_range: input.time_range,
        adapter_mode,
        executed_query: query,
        entries,
        clusters,
        summary,
    })
}

fn resolve_adapter_mode(connection: &Connection, environment_id: &str) -> Result<String, String> {
    let elk_profile = db::list_connection_profiles(connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|profile| profile.environment_id == environment_id && profile.profile_type == "elk");

    Ok(match elk_profile {
        Some(ConnectionProfile { endpoint, .. }) if !endpoint.trim().is_empty() => {
            format!("elk-profile-configured ({endpoint})")
        }
        _ => "mock-elastic-adapter".to_string(),
    })
}

fn build_query_string(input: &LogSearchInput) -> String {
    let mut parts = vec![
        format!("environment={}", input.environment_id),
        format!("timeRange={}", input.time_range),
    ];

    if let Some(service) = clean_filter(&input.service) {
        parts.push(format!("service={service}"));
    }
    if let Some(pod) = clean_filter(&input.pod) {
        parts.push(format!("pod={pod}"));
    }
    if let Some(keyword) = clean_filter(&input.keyword) {
        parts.push(format!("keyword={keyword}"));
    }
    if let Some(trace_id) = clean_filter(&input.trace_id) {
        parts.push(format!("traceId={trace_id}"));
    }

    parts.join(" AND ")
}

fn filter_entries(entries: Vec<LogEntry>, input: &LogSearchInput) -> Vec<LogEntry> {
    let service = clean_filter(&input.service).map(|value| value.to_ascii_lowercase());
    let pod = clean_filter(&input.pod).map(|value| value.to_ascii_lowercase());
    let keyword = clean_filter(&input.keyword).map(|value| value.to_ascii_lowercase());
    let trace_id = clean_filter(&input.trace_id).map(|value| value.to_ascii_lowercase());
    let now = Utc::now();
    let threshold = now - duration_for_time_range(&input.time_range);

    entries
        .into_iter()
        .filter(|entry| entry.environment_id == input.environment_id)
        .filter(|entry| {
            chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
                .map(|timestamp| timestamp.with_timezone(&Utc) >= threshold)
                .unwrap_or(true)
        })
        .filter(|entry| {
            service
                .as_ref()
                .map(|value| entry.service.to_ascii_lowercase().contains(value))
                .unwrap_or(true)
        })
        .filter(|entry| {
            pod.as_ref()
                .map(|value| entry.pod.to_ascii_lowercase().contains(value))
                .unwrap_or(true)
        })
        .filter(|entry| {
            keyword
                .as_ref()
                .map(|value| entry.message.to_ascii_lowercase().contains(value))
                .unwrap_or(true)
        })
        .filter(|entry| {
            trace_id
                .as_ref()
                .map(|value| {
                    entry.trace_id
                        .as_ref()
                        .map(|trace| trace.to_ascii_lowercase().contains(value))
                        .unwrap_or(false)
                })
                .unwrap_or(true)
        })
        .collect()
}

fn duration_for_time_range(time_range: &str) -> Duration {
    match time_range {
        "15m" => Duration::minutes(15),
        "1h" => Duration::hours(1),
        "6h" => Duration::hours(6),
        "24h" => Duration::hours(24),
        _ => Duration::hours(1),
    }
}

fn cluster_entries(entries: &[LogEntry]) -> Vec<LogCluster> {
    let mut grouped: HashMap<String, Vec<&LogEntry>> = HashMap::new();

    for entry in entries {
        grouped
            .entry(normalize_message(&entry.message))
            .or_default()
            .push(entry);
    }

    let mut clusters = grouped
        .into_iter()
        .map(|(key, grouped_entries)| {
            let first = grouped_entries[0];
            let services = grouped_entries
                .iter()
                .map(|entry| entry.service.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            LogCluster {
                id: key.replace(' ', "-"),
                label: cluster_label(&first.message),
                level: first.level.clone(),
                count: grouped_entries.len(),
                services,
                example_message: first.message.clone(),
                trace_id: first.trace_id.clone(),
            }
        })
        .collect::<Vec<_>>();

    clusters.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    clusters
}

fn normalize_message(message: &str) -> String {
    message
        .split_whitespace()
        .map(|token| {
            if token.chars().any(|char| char.is_ascii_digit()) {
                "<value>"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn cluster_label(message: &str) -> String {
    let trimmed = message.trim();
    let preview = trimmed.chars().take(64).collect::<String>();
    if trimmed.chars().count() > 64 {
        format!("{preview}...")
    } else {
        preview
    }
}

fn build_summary(
    input: &LogSearchInput,
    entries: &[LogEntry],
    clusters: &[LogCluster],
) -> LogSummary {
    if entries.is_empty() {
        return LogSummary {
            headline: format!(
                "No log events matched the current filters in {} for {}.",
                input.environment_id, input.time_range
            ),
            likely_causes: vec![
                "The filter is too narrow, especially service, pod, or traceId.".to_string(),
                "The incident may be outside the selected time range.".to_string(),
            ],
            recommended_next_steps: vec![
                "Expand the time range to 6h or 24h.".to_string(),
                "Retry without pod and traceId filters to find the broader error pattern.".to_string(),
            ],
        };
    }

    let error_count = entries.iter().filter(|entry| entry.level == "ERROR").count();
    let warn_count = entries.iter().filter(|entry| entry.level == "WARN").count();
    let top_cluster = clusters.first();
    let top_cluster_text = top_cluster
        .map(|cluster| format!("Top cluster: {} ({})", cluster.label, cluster.count))
        .unwrap_or_else(|| "No dominant cluster yet.".to_string());

    let mut likely_causes = Vec::new();
    if let Some(cluster) = top_cluster {
        likely_causes.push(format!(
            "{} is the dominant repeated failure pattern across {} service(s).",
            cluster.label,
            cluster.services.len()
        ));
    }
    if error_count > warn_count {
        likely_causes.push("The result set is error-heavy, which suggests an active failure rather than startup noise.".to_string());
    } else {
        likely_causes.push("Warnings are still prominent, which can indicate a degrading dependency before full failure.".to_string());
    }

    let mut next_steps = vec![
        "Attach the dominant cluster to chat so the assistant can correlate it with pods and recent deploys.".to_string(),
        "Pivot on the traceId or pod from the hottest sample to isolate one failing request path.".to_string(),
    ];
    if let Some(service) = clean_filter(&input.service) {
        next_steps.push(format!(
            "Compare {service} logs with Kubernetes events and restart counts in the same window."
        ));
    } else {
        next_steps.push("Add a service filter after the first broad pass to tighten the error narrative.".to_string());
    }

    LogSummary {
        headline: format!(
            "{} matching events in {}. {}",
            entries.len(),
            input.time_range,
            top_cluster_text
        ),
        likely_causes,
        recommended_next_steps: next_steps,
    }
}

fn clean_filter(value: &Option<String>) -> Option<String> {
    value.as_ref()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

fn sample_entries() -> Vec<LogEntry> {
    let now = Utc::now();

    vec![
        log_entry(
            now - Duration::minutes(9),
            "prod",
            "payment-api",
            "payment-api-6b7c9c8d4f-29kqm",
            "ERROR",
            Some("trace-pay-1842"),
            "Redis command timeout after 2000ms while loading checkout session 1842",
        ),
        log_entry(
            now - Duration::minutes(8),
            "prod",
            "payment-api",
            "payment-api-6b7c9c8d4f-29kqm",
            "ERROR",
            Some("trace-pay-1843"),
            "Redis command timeout after 2000ms while loading checkout session 1843",
        ),
        log_entry(
            now - Duration::minutes(7),
            "prod",
            "payment-worker",
            "payment-worker-77c88c4f8f-k2xnl",
            "WARN",
            Some("trace-pay-1843"),
            "Downstream inventory request exceeded 1500ms threshold for order 59201",
        ),
        log_entry(
            now - Duration::minutes(6),
            "prod",
            "gateway",
            "gateway-75bc9f7d6d-zx6qt",
            "ERROR",
            Some("trace-gw-5530"),
            "Upstream payment-api returned HTTP 502 for request 5530",
        ),
        log_entry(
            now - Duration::minutes(4),
            "prod",
            "payment-api",
            "payment-api-6b7c9c8d4f-xh4nm",
            "ERROR",
            Some("trace-pay-1844"),
            "Redis command timeout after 2000ms while loading checkout session 1844",
        ),
        log_entry(
            now - Duration::minutes(42),
            "test",
            "payment-api",
            "payment-api-54d95f7b8d-gv6dm",
            "WARN",
            Some("trace-test-991"),
            "Redis connection pool saturation warning while preparing checkout 991",
        ),
        log_entry(
            now - Duration::minutes(38),
            "test",
            "order-api",
            "order-api-6c5d889db8-2spxw",
            "ERROR",
            Some("trace-order-881"),
            "Nacos config fetch failed for dataId order-service.yaml in DEFAULT_GROUP",
        ),
        log_entry(
            now - Duration::minutes(18),
            "dev",
            "payment-api",
            "payment-api-7d6f7fcf66-vw4lp",
            "INFO",
            Some("trace-dev-101"),
            "Health check completed in 34ms for checkout request 101",
        ),
        log_entry(
            now - Duration::minutes(11),
            "dev",
            "gateway",
            "gateway-7f9fcb4b78-vrghp",
            "WARN",
            Some("trace-dev-309"),
            "Feature flag checkout.retry enabled for tenant 309",
        ),
    ]
}

fn log_entry(
    timestamp: chrono::DateTime<Utc>,
    environment_id: &str,
    service: &str,
    pod: &str,
    level: &str,
    trace_id: Option<&str>,
    message: &str,
) -> LogEntry {
    LogEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: timestamp.to_rfc3339(),
        environment_id: environment_id.to_string(),
        service: service.to_string(),
        pod: pod.to_string(),
        level: level.to_string(),
        trace_id: trace_id.map(str::to_string),
        message: message.to_string(),
    }
}
