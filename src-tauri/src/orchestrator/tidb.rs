use std::path::Path;

use chrono::{Duration, SecondsFormat, Utc};
use mysql::prelude::Queryable;
use mysql::{Opts, OptsBuilder, Pool, PooledConn, Row, Value as MyValue};
use rusqlite::Connection;
use serde_json::Value;

use crate::hardening::sanitize_and_mask_text;
use crate::models::connection_profile::ConnectionProfile;
use crate::models::tidb::{
    AnalyzeTidbInput, AnalyzeTidbResponse, TidbMetric, TidbSlowQuery, TidbSummary,
};
use crate::storage::{db, secrets};

pub fn analyze_tidb(
    connection: &Connection,
    app_data_dir: &str,
    input: AnalyzeTidbInput,
) -> Result<AnalyzeTidbResponse, String> {
    let profile = resolve_profile(connection, &input.environment_id)?
        .ok_or_else(|| format!("No TiDB profile found for environment {}. Add one in Settings first.", input.environment_id))?;
    let config = TidbProfileConfig::from_profile(&profile)?;
    let password = if profile.has_secret {
        Some(
            secrets::get_profile_secret(Some(Path::new(app_data_dir)), &profile.id)
                .map_err(|error| format!("Failed to load TiDB secret for profile '{}': {}", profile.name, error))?,
        )
    } else {
        None
    };

    let instance_name = input
        .instance_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| profile.name.clone());
    let limit = input.slow_query_limit.unwrap_or(config.slow_query_limit).clamp(1, 200);
    let since = normalize_time_range(&input.time_range)?;

    let opts = build_connection_options(&profile, &config, password.as_deref())?;
    let pool = Pool::new(opts).map_err(|error| format!("Failed to open TiDB connection pool: {error}"))?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Failed to connect to TiDB '{}': {error}", profile.endpoint.trim()))?;

    let (source_relation, slow_queries) = load_slow_queries(&mut conn, since, limit)?;
    let metrics = build_metrics(&slow_queries, limit, &source_relation);
    let executed_plan = format!(
        "Connect to {} -> query {} for slow SQL since {} -> limit {}",
        profile.endpoint.trim(),
        source_relation,
        since.to_rfc3339_opts(SecondsFormat::Secs, true),
        limit
    );
    let summary = build_summary(&instance_name, &slow_queries, &metrics);

    Ok(AnalyzeTidbResponse {
        environment_id: input.environment_id,
        instance_name,
        time_range: input.time_range,
        adapter_mode: format!("tidb-mysql-direct ({})", profile.endpoint.trim()),
        executed_plan,
        source_relation,
        metrics,
        slow_queries,
        summary,
    })
}

fn resolve_profile(connection: &Connection, environment_id: &str) -> Result<Option<ConnectionProfile>, String> {
    db::list_connection_profiles(connection)
        .map_err(|error| error.to_string())
        .map(|profiles| {
            profiles
                .into_iter()
                .find(|profile| profile.environment_id == environment_id && profile.profile_type == "tidb")
        })
}

fn normalize_time_range(value: &str) -> Result<chrono::DateTime<Utc>, String> {
    let minutes = match value.trim() {
        "15m" => 15,
        "1h" => 60,
        "6h" => 360,
        "24h" => 1440,
        other => return Err(format!("Unsupported TiDB time range: {other}")),
    };
    Ok(Utc::now() - Duration::minutes(minutes))
}

fn build_connection_options(
    profile: &ConnectionProfile,
    config: &TidbProfileConfig,
    password: Option<&str>,
) -> Result<Opts, String> {
    let (host, port) = parse_host_port(&profile.endpoint)?;
    let mut builder = OptsBuilder::new()
        .ip_or_hostname(Some(host))
        .tcp_port(port)
        .db_name(Some(config.database.clone()))
        .stmt_cache_size(Some(0));

    if let Some(username) = profile.username.as_deref().filter(|value| !value.trim().is_empty()) {
        builder = builder.user(Some(username.to_string()));
    }
    if let Some(password) = password.filter(|value| !value.trim().is_empty()) {
        builder = builder.pass(Some(password.to_string()));
    }

    Ok(Opts::from(builder))
}

fn parse_host_port(endpoint: &str) -> Result<(String, u16), String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err("TiDB endpoint is required. Use host:port in Settings.".to_string());
    }
    let without_scheme = trimmed
        .strip_prefix("mysql://")
        .or_else(|| trimmed.strip_prefix("tidb://"))
        .unwrap_or(trimmed);
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme).trim();
    match host_port.rsplit_once(':') {
        Some((host, port_text)) => {
            let port = port_text
                .parse::<u16>()
                .map_err(|_| format!("Invalid TiDB port in endpoint: {trimmed}"))?;
            Ok((host.to_string(), port))
        }
        None => Ok((host_port.to_string(), 4000)),
    }
}

fn load_slow_queries(
    conn: &mut PooledConn,
    since: chrono::DateTime<Utc>,
    limit: u32,
) -> Result<(String, Vec<TidbSlowQuery>), String> {
    let candidates = [
        "information_schema.CLUSTER_SLOW_QUERY",
        "information_schema.SLOW_QUERY",
    ];
    let since_text = since.naive_utc().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut last_error = None;
    for relation in candidates {
        let sql = format!(
            "SELECT TIME, QUERY_TIME, DIGEST, DB, USER, INDEX_NAMES, QUERY \
             FROM {relation} \
             WHERE TIME >= ? \
             ORDER BY QUERY_TIME DESC \
             LIMIT ?"
        );
        match conn.exec::<Row, _, _>(sql, (since_text.as_str(), limit)) {
            Ok(rows) => {
                let mapped = rows
                    .into_iter()
                    .enumerate()
                    .map(|(index, row)| map_slow_query(index, row))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok((relation.to_string(), mapped));
            }
            Err(error) => last_error = Some(format!("{relation}: {error}")),
        }
    }

    Err(last_error.unwrap_or_else(|| "No TiDB slow query relation could be queried.".to_string()))
}

fn map_slow_query(index: usize, row: Row) -> Result<TidbSlowQuery, String> {
    let mut values = row.unwrap();
    if values.len() < 7 {
        return Err("TiDB slow query row did not contain the expected columns.".to_string());
    }
    let timestamp = mysql_value_to_string(values.remove(0));
    let query_time_secs = mysql_value_to_f64(values.remove(0));
    let digest = mysql_value_to_string(values.remove(0));
    let database_name = mysql_value_to_string(values.remove(0));
    let user = mysql_value_to_string(values.remove(0));
    let index_names = mysql_value_to_string(values.remove(0));
    let query_text = sanitize_and_mask_text(&mysql_value_to_string(values.remove(0)));

    Ok(TidbSlowQuery {
        id: format!("tidb-slow-{index}"),
        timestamp,
        query_time_secs,
        digest: if digest.is_empty() { "n/a".to_string() } else { digest },
        database_name: if database_name.is_empty() { "unknown".to_string() } else { database_name },
        user: if user.is_empty() { "unknown".to_string() } else { user },
        index_names: if index_names.is_empty() { "n/a".to_string() } else { index_names },
        query_text,
    })
}

fn build_metrics(queries: &[TidbSlowQuery], limit: u32, source_relation: &str) -> Vec<TidbMetric> {
    let slow_count = queries.len();
    let max_time = queries
        .iter()
        .map(|query| query.query_time_secs)
        .fold(0.0_f64, f64::max);
    let avg_time = if slow_count > 0 {
        queries.iter().map(|query| query.query_time_secs).sum::<f64>() / slow_count as f64
    } else {
        0.0
    };
    let unique_digests = queries
        .iter()
        .map(|query| query.digest.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    vec![
        TidbMetric {
            label: "slow_query_rows".to_string(),
            value: format!("{slow_count}/{limit}"),
            status: if slow_count as u32 >= limit { "warning" } else { "healthy" }.to_string(),
            detail: format!("Rows returned from {source_relation} within the selected time window."),
        },
        TidbMetric {
            label: "max_query_time".to_string(),
            value: format!("{max_time:.3}s"),
            status: if max_time >= 5.0 { "danger" } else if max_time >= 1.0 { "warning" } else { "healthy" }.to_string(),
            detail: "Longest query_time observed in the current sample.".to_string(),
        },
        TidbMetric {
            label: "avg_query_time".to_string(),
            value: format!("{avg_time:.3}s"),
            status: if avg_time >= 2.0 { "warning" } else { "healthy" }.to_string(),
            detail: "Average query_time across the collected slow SQL sample.".to_string(),
        },
        TidbMetric {
            label: "unique_digests".to_string(),
            value: unique_digests.to_string(),
            status: if unique_digests >= 10 { "warning" } else { "healthy" }.to_string(),
            detail: "Distinct SQL digests help show whether the issue is concentrated or broad.".to_string(),
        },
    ]
}

fn build_summary(instance_name: &str, queries: &[TidbSlowQuery], metrics: &[TidbMetric]) -> TidbSummary {
    let top_query = queries
        .first()
        .map(|query| summarize_sql(&query.query_text))
        .unwrap_or_else(|| "no slow SQL captured".to_string());
    let warning_metrics = metrics.iter().filter(|metric| metric.status != "healthy").count();
    let headline = format!(
        "{instance_name} returned {} slow SQL row(s) with {} warning metric(s); top statement: {top_query}.",
        queries.len(),
        warning_metrics
    );

    TidbSummary {
        headline,
        likely_causes: vec![
            "The top slow SQL may be missing an efficient index path or scanning too many rows.".to_string(),
            "Repeated digests often indicate a hot code path, batch worker, or dashboard query driving cluster pressure.".to_string(),
            "Large query_time values can also reflect lock wait, TiKV hotspotting, or stale execution plans.".to_string(),
        ],
        recommended_next_steps: vec![
            "Run EXPLAIN ANALYZE on the worst digest and compare index choice, rows, and operator timing.".to_string(),
            "Correlate the slow SQL timestamp with application releases, traffic spikes, and Redis or upstream latency evidence.".to_string(),
            "Group repeated digests by service owner so the LLM or incident report can point to the hottest application path.".to_string(),
        ],
    }
}

fn summarize_sql(sql: &str) -> String {
    let compact = sql.split_whitespace().take(12).collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        "unknown SQL".to_string()
    } else {
        compact
    }
}

fn mysql_value_to_string(value: MyValue) -> String {
    match value {
        MyValue::NULL => String::new(),
        MyValue::Bytes(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        MyValue::Int(number) => number.to_string(),
        MyValue::UInt(number) => number.to_string(),
        MyValue::Float(number) => number.to_string(),
        MyValue::Double(number) => number.to_string(),
        MyValue::Date(year, month, day, hour, minute, second, micros) => {
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{:06}", micros)
        }
        MyValue::Time(_, days, hours, minutes, seconds, micros) => {
            let total_hours = days * 24 + u32::from(hours);
            format!("{total_hours:02}:{minutes:02}:{seconds:02}.{:06}", micros)
        }
    }
}

fn mysql_value_to_f64(value: MyValue) -> f64 {
    match value {
        MyValue::NULL => 0.0,
        MyValue::Float(number) => number as f64,
        MyValue::Double(number) => number,
        MyValue::Int(number) => number as f64,
        MyValue::UInt(number) => number as f64,
        MyValue::Bytes(bytes) => String::from_utf8_lossy(&bytes).parse::<f64>().unwrap_or(0.0),
        other => mysql_value_to_string(other).parse::<f64>().unwrap_or(0.0),
    }
}

#[derive(Debug, Clone)]
struct TidbProfileConfig {
    database: String,
    slow_query_limit: u32,
}

impl TidbProfileConfig {
    fn from_profile(profile: &ConnectionProfile) -> Result<Self, String> {
        let config_json = if profile.config_json.trim().is_empty() {
            "{}"
        } else {
            profile.config_json.as_str()
        };
        let config = serde_json::from_str::<Value>(config_json)
            .map_err(|error| format!("Invalid TiDB profile JSON: {error}"))?;
        let database = config
            .get("database")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("mysql")
            .to_string();
        let slow_query_limit = config
            .get("slowQueryLimit")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(20);

        Ok(Self {
            database,
            slow_query_limit,
        })
    }
}
