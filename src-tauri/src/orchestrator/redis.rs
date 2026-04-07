use chrono::{Duration, Utc};
use rusqlite::Connection;
use uuid::Uuid;

use crate::models::connection_profile::ConnectionProfile;
use crate::models::redis::{
    AnalyzeRedisInput, AnalyzeRedisResponse, RedisInfoMetric, RedisLatencyPoint, RedisLogLine,
    RedisSlowQuery, RedisSummary,
};
use crate::storage::db;

pub fn analyze_redis(
    connection: &Connection,
    input: AnalyzeRedisInput,
) -> Result<AnalyzeRedisResponse, String> {
    let profile = resolve_profile(connection, &input.environment_id);
    let instance_name = input
        .instance_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| profile.as_ref().map(|item| item.name.clone()))
        .unwrap_or_else(|| format!("{}-redis-main", input.environment_id));
    let adapter_mode = match profile {
        Some(ConnectionProfile { endpoint, .. }) if !endpoint.trim().is_empty() => {
            format!("redis-profile-configured ({endpoint})")
        }
        _ => "mock-redis-adapter".to_string(),
    };
    let executed_plan = format!(
        "INFO all -> latency sample window {} -> SLOWLOG GET 5 -> recent Redis log scan",
        input.time_range
    );

    let info_metrics = sample_info_metrics(&instance_name);
    let slow_queries = sample_slow_queries(&input.environment_id, &instance_name, &input.time_range);
    let latency_points = sample_latency(&input.time_range);
    let log_lines = sample_log_lines(&instance_name);
    let summary = build_summary(&instance_name, &info_metrics, &slow_queries, &latency_points, &log_lines);

    Ok(AnalyzeRedisResponse {
        environment_id: input.environment_id,
        instance_name,
        time_range: input.time_range,
        adapter_mode,
        executed_plan,
        info_metrics,
        slow_queries,
        latency_points,
        log_lines,
        summary,
    })
}

fn resolve_profile(connection: &Connection, environment_id: &str) -> Option<ConnectionProfile> {
    db::list_connection_profiles(connection)
        .ok()?
        .into_iter()
        .find(|profile| profile.environment_id == environment_id && profile.profile_type == "redis")
}

fn sample_info_metrics(instance_name: &str) -> Vec<RedisInfoMetric> {
    vec![
        RedisInfoMetric {
            label: "uptime".to_string(),
            value: "18d 06h".to_string(),
            status: "healthy".to_string(),
            detail: format!("{instance_name} has been stable since the last planned restart."),
        },
        RedisInfoMetric {
            label: "used_memory".to_string(),
            value: "8.1 GB / 10 GB".to_string(),
            status: "warning".to_string(),
            detail: "Memory is above 80 percent and eviction pressure is increasing.".to_string(),
        },
        RedisInfoMetric {
            label: "connected_clients".to_string(),
            value: "482".to_string(),
            status: "healthy".to_string(),
            detail: "Client volume is elevated but still within the expected production band.".to_string(),
        },
        RedisInfoMetric {
            label: "blocked_clients".to_string(),
            value: "3".to_string(),
            status: "warning".to_string(),
            detail: "Blocked clients suggest heavy scripts or slow downstream commands.".to_string(),
        },
        RedisInfoMetric {
            label: "keyspace_hits".to_string(),
            value: "94.7%".to_string(),
            status: "healthy".to_string(),
            detail: "Cache efficiency is still strong, so latency is more likely command-related than miss-related.".to_string(),
        },
    ]
}

fn sample_slow_queries(environment_id: &str, instance_name: &str, time_range: &str) -> Vec<RedisSlowQuery> {
    let now = Utc::now();
    let minutes = match time_range {
        "15m" => 15,
        "1h" => 60,
        "6h" => 360,
        "24h" => 1440,
        _ => 60,
    };
    let base = now - Duration::minutes((minutes / 2) as i64);

    vec![
        RedisSlowQuery {
            id: Uuid::new_v4().to_string(),
            timestamp: (base + Duration::minutes(4)).to_rfc3339(),
            duration_micros: 182_340,
            command: "EVALSHA".to_string(),
            key_sample: format!("{environment_id}:{instance_name}:checkout:lock"),
            client: "10.0.14.22:53418".to_string(),
        },
        RedisSlowQuery {
            id: Uuid::new_v4().to_string(),
            timestamp: (base + Duration::minutes(11)).to_rfc3339(),
            duration_micros: 143_820,
            command: "ZRANGEBYSCORE".to_string(),
            key_sample: format!("{environment_id}:{instance_name}:queue:delayed"),
            client: "10.0.14.34:53802".to_string(),
        },
        RedisSlowQuery {
            id: Uuid::new_v4().to_string(),
            timestamp: (base + Duration::minutes(15)).to_rfc3339(),
            duration_micros: 110_450,
            command: "HGETALL".to_string(),
            key_sample: format!("{environment_id}:{instance_name}:session:984221"),
            client: "10.0.14.11:52917".to_string(),
        },
    ]
}

fn sample_latency(time_range: &str) -> Vec<RedisLatencyPoint> {
    let now = Utc::now();
    let step_minutes: i64 = match time_range {
        "15m" => 3,
        "1h" => 10,
        "6h" => 60,
        "24h" => 180,
        _ => 10,
    };

    (0..6)
        .map(|index| RedisLatencyPoint {
            timestamp: (now - Duration::minutes(step_minutes * (5 - index))).to_rfc3339(),
            avg_ms: 4.0 + (index as f64 * 1.7),
            p95_ms: 14.0 + (index as f64 * 8.4),
        })
        .collect()
}

fn sample_log_lines(instance_name: &str) -> Vec<RedisLogLine> {
    let now = Utc::now();

    vec![
        RedisLogLine {
            timestamp: (now - Duration::minutes(16)).to_rfc3339(),
            level: "NOTICE".to_string(),
            message: format!("{instance_name} memory usage crossed 80% of maxmemory."),
        },
        RedisLogLine {
            timestamp: (now - Duration::minutes(12)).to_rfc3339(),
            level: "WARNING".to_string(),
            message: "Latency monitor reported command spikes above 120ms.".to_string(),
        },
        RedisLogLine {
            timestamp: (now - Duration::minutes(8)).to_rfc3339(),
            level: "WARNING".to_string(),
            message: "Lua script execution time exceeded configured slowlog threshold.".to_string(),
        },
        RedisLogLine {
            timestamp: (now - Duration::minutes(4)).to_rfc3339(),
            level: "NOTICE".to_string(),
            message: "Replication link healthy, no role change detected.".to_string(),
        },
    ]
}

fn build_summary(
    instance_name: &str,
    info_metrics: &[RedisInfoMetric],
    slow_queries: &[RedisSlowQuery],
    latency_points: &[RedisLatencyPoint],
    log_lines: &[RedisLogLine],
) -> RedisSummary {
    let warning_metrics = info_metrics
        .iter()
        .filter(|metric| metric.status != "healthy")
        .count();
    let top_slow = slow_queries
        .iter()
        .max_by_key(|query| query.duration_micros)
        .map(|query| format!("{} at {}us", query.command, query.duration_micros))
        .unwrap_or_else(|| "no recent slow queries".to_string());
    let max_p95 = latency_points
        .iter()
        .map(|point| point.p95_ms)
        .fold(0.0_f64, f64::max);
    let warning_logs = log_lines
        .iter()
        .filter(|line| line.level == "WARNING")
        .count();

    RedisSummary {
        headline: format!(
            "{instance_name} shows {warning_metrics} warning metric(s), p95 latency peaking at {max_p95:.1}ms, with top slow command {top_slow}."
        ),
        likely_causes: vec![
            "Memory pressure is rising, which can amplify latency and blocked clients during bursts.".to_string(),
            "Slow Lua or sorted-set commands are the strongest contributors to the current response-time spikes.".to_string(),
            format!(
                "{warning_logs} warning log entries line up with the latency spike window, so Redis itself is showing distress instead of only the application."
            ),
        ],
        recommended_next_steps: vec![
            "Inspect big keys and Lua script frequency before increasing maxmemory.".to_string(),
            "Correlate the slowlog keys with the application path or queue worker that owns them.".to_string(),
            "Compare this latency window with app-side timeout errors in ELK and upstream service retries.".to_string(),
        ],
    }
}
