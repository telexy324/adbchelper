use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use chrono::{Duration, Utc};
use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::hardening::{sanitize_and_mask_text, sanitize_untrusted_text};
use crate::models::connection_profile::ConnectionProfile;
use crate::models::logs::{LogCluster, LogEntry, LogSearchInput, LogSearchResponse, LogSummary};
use crate::storage::secrets;

pub async fn search_logs(
    profile: Option<ConnectionProfile>,
    app_data_dir: &str,
    input: LogSearchInput,
) -> Result<LogSearchResponse, String> {
    let query = build_query_string(&input);

    let (adapter_mode, entries) = match profile {
        Some(profile) => {
            let adapter_mode = format!("elk-http-search ({})", profile.endpoint);
            let entries = fetch_elk_entries(&profile, app_data_dir, &input).await?;
            (adapter_mode, entries)
        }
        None => {
            let entries = filter_entries(sample_entries(), &input);
            ("mock-elastic-adapter".to_string(), entries)
        }
    };

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

async fn fetch_elk_entries(
    profile: &ConnectionProfile,
    app_data_dir: &str,
    input: &LogSearchInput,
) -> Result<Vec<LogEntry>, String> {
    let config = ElkProfileConfig::from_profile(profile)?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| format!("Failed to build ELK HTTP client: {error}"))?;
    let mut request = client
        .post(config.search_url(&profile.endpoint))
        .header("Content-Type", "application/json")
        .json(&build_elk_query(input, &config));

    if let Some(space) = config.space.as_deref() {
        request = request.header("kbn-space", space);
    }

    if let Some(username) = profile.username.as_deref().filter(|value| !value.trim().is_empty()) {
        let password = secrets::get_profile_secret(Some(Path::new(app_data_dir)), &profile.id)
            .map_err(|error| format!("Failed to load ELK secret for profile '{}': {}", profile.name, error))?;
        request = request.basic_auth(username.to_string(), Some(password));
    } else if profile.has_secret {
        let token = secrets::get_profile_secret(Some(Path::new(app_data_dir)), &profile.id)
            .map_err(|error| format!("Failed to load ELK secret for profile '{}': {}", profile.name, error))?;
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("ELK request failed for {}: {error}", profile.name))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "ELK request failed for {} with status {}: {}",
            profile.name,
            status,
            sanitize_and_mask_text(body.trim())
        ));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|error| format!("Invalid ELK response for {}: {error}", profile.name))?;

    Ok(parse_elk_hits(&payload, &input.environment_id))
}

fn build_elk_query(input: &LogSearchInput, config: &ElkProfileConfig) -> Value {
    let mut filters = vec![json!({
        "range": {
            config.timestamp_field.as_str(): {
                "gte": format!("now-{}", input.time_range),
                "lte": "now"
            }
        }
    })];

    if let Some(service) = clean_filter(&input.service) {
        filters.push(text_filter(&config.service_field, &service));
    }
    if let Some(pod) = clean_filter(&input.pod) {
        filters.push(text_filter(&config.pod_field, &pod));
    }
    if let Some(keyword) = clean_filter(&input.keyword) {
        filters.push(json!({
            "multi_match": {
                "query": keyword,
                "fields": [
                    config.message_field.as_str(),
                    config.service_field.as_str(),
                    config.pod_field.as_str(),
                    config.trace_id_field.as_str()
                ]
            }
        }));
    }
    if let Some(trace_id) = clean_filter(&input.trace_id) {
        filters.push(text_filter(&config.trace_id_field, &trace_id));
    }

    json!({
        "size": 200,
        "sort": [
            {
                config.timestamp_field.as_str(): {
                    "order": "desc"
                }
            }
        ],
        "_source": [
            config.timestamp_field.as_str(),
            config.message_field.as_str(),
            config.level_field.as_str(),
            config.service_field.as_str(),
            config.pod_field.as_str(),
            config.trace_id_field.as_str(),
            "traceId",
            "trace_id",
            "kubernetes.pod.name",
            "service.name",
            "log.level",
            "message"
        ],
        "query": {
            "bool": {
                "filter": filters
            }
        }
    })
}

fn text_filter(field: &str, value: &str) -> Value {
    json!({
        "query_string": {
            "default_field": field,
            "query": format!("*{}*", escape_query_string(value))
        }
    })
}

fn escape_query_string(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '+' | '-' | '=' | '&' | '|' | '>' | '<' | '!' | '(' | ')' | '{' | '}' | '[' | ']'
            | '^' | '"' | '~' | '*' | '?' | ':' | '\\' | '/' => ['\\', ch].into_iter().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}

fn parse_elk_hits(payload: &Value, environment_id: &str) -> Vec<LogEntry> {
    payload
        .get("hits")
        .and_then(|value| value.get("hits"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|hit| {
            let source = hit.get("_source")?;
            let message = first_string(
                source,
                &[
                    "message",
                    "log.message",
                    "error.message",
                    "event.original",
                ],
            )
            .unwrap_or_else(|| "".to_string());
            if message.trim().is_empty() {
                return None;
            }

            let timestamp = first_string(source, &["@timestamp", "timestamp", "time"])
                .unwrap_or_else(|| Utc::now().to_rfc3339());
            let service = first_string(source, &["service.name", "service", "app", "application"])
                .unwrap_or_else(|| "unknown-service".to_string());
            let pod = first_string(source, &["kubernetes.pod.name", "pod", "pod_name"])
                .unwrap_or_else(|| "unknown-pod".to_string());
            let level = first_string(source, &["log.level", "level", "severity"])
                .unwrap_or_else(|| "INFO".to_string())
                .to_ascii_uppercase();
            let trace_id = first_string(
                source,
                &[
                    "traceId",
                    "trace_id",
                    "trace.id",
                    "mdc.traceId",
                    "labels.traceId",
                ],
            );

            Some(LogEntry {
                id: hit
                    .get("_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
                timestamp,
                environment_id: environment_id.to_string(),
                service: sanitize_and_mask_text(&service),
                pod: sanitize_and_mask_text(&pod),
                level,
                trace_id: trace_id.map(|value| sanitize_and_mask_text(&value)),
                message: sanitize_untrusted_text(&message),
            })
        })
        .collect()
}

fn first_string(value: &Value, paths: &[&str]) -> Option<String> {
    for path in paths {
        if let Some(found) = dotted_lookup(value, path).and_then(value_to_string) {
            if !found.trim().is_empty() {
                return Some(found);
            }
        }
    }
    None
}

fn dotted_lookup<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Array(items) => items.first().and_then(value_to_string),
        _ => None,
    }
}

struct ElkProfileConfig {
    index_pattern: String,
    timestamp_field: String,
    message_field: String,
    level_field: String,
    service_field: String,
    pod_field: String,
    trace_id_field: String,
    space: Option<String>,
}

impl ElkProfileConfig {
    fn from_profile(profile: &ConnectionProfile) -> Result<Self, String> {
        let config = serde_json::from_str::<Value>(&profile.config_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));

        let index_pattern = config
            .get("indexPattern")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("logs-*")
            .to_string();

        Ok(Self {
            index_pattern,
            timestamp_field: string_or_default(&config, "timestampField", "@timestamp"),
            message_field: string_or_default(&config, "messageField", "message"),
            level_field: string_or_default(&config, "levelField", "log.level"),
            service_field: string_or_default(&config, "serviceField", "service.name"),
            pod_field: string_or_default(&config, "podField", "kubernetes.pod.name"),
            trace_id_field: string_or_default(&config, "traceIdField", "traceId"),
            space: config
                .get("space")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        })
    }

    fn search_url(&self, endpoint: &str) -> String {
        let trimmed = endpoint.trim_end_matches('/');
        format!("{}/{}/_search", trimmed, self.index_pattern)
    }
}

fn string_or_default(config: &Value, key: &str, default: &str) -> String {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
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
            Some("trace-test-221"),
            "Nacos config refresh took 420ms for dataId payment-service.yaml",
        ),
        log_entry(
            now - Duration::minutes(39),
            "test",
            "gateway",
            "gateway-5cf7d459fd-7qk8f",
            "ERROR",
            Some("trace-test-223"),
            "Downstream payment-api returned HTTP 504 for request 223",
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
