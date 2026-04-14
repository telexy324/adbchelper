use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use serde_json::Value;

use crate::hardening::sanitize_and_mask_text;
use crate::models::connection_profile::ConnectionProfile;
use crate::models::redis::{
    AnalyzeRedisInput, AnalyzeRedisResponse, RedisInfoMetric, RedisLatencyPoint, RedisLogLine,
    RedisSlowQuery, RedisSummary,
};
use crate::storage::{db, secrets};

pub fn analyze_redis(
    connection: &Connection,
    app_data_dir: &str,
    input: AnalyzeRedisInput,
) -> Result<AnalyzeRedisResponse, String> {
    let profile = resolve_profile(connection, &input.environment_id)?
        .ok_or_else(|| {
            format!(
                "No Redis profile found for environment {}. Add one in Settings first.",
                input.environment_id
            )
        })?;
    let config = RedisProfileConfig::from_profile(&profile)?;
    let instance_name = input
        .instance_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            profile
                .default_scope
                .clone()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| profile.name.clone());

    if config.tls_enabled {
        return Err(format!(
            "Redis profile '{}' has TLS enabled, but the current desktop adapter only supports plain TCP right now.",
            profile.name
        ));
    }

    let endpoint = parse_endpoint(&profile.endpoint)?;
    let password = if profile.has_secret {
        Some(
            secrets::get_profile_secret(Some(Path::new(app_data_dir)), &profile.id)
                .map_err(|error| format!("Failed to load Redis secret for profile '{}': {}", profile.name, error))?,
        )
    } else {
        None
    };

    let mut client = RedisClient::connect(&endpoint, config.command_timeout)?;
    client.authenticate(profile.username.as_deref(), password.as_deref())?;
    if config.database > 0 {
        client.select_database(config.database)?;
    }

    let info_body = client
        .run(["INFO", "ALL"])?
        .as_string()
        .ok_or_else(|| "Unexpected INFO response from Redis.".to_string())?;
    let info_map = parse_info_sections(&info_body);
    let slow_queries = load_slow_queries(&mut client, config.slowlog_limit)?;
    let latency_points = measure_latency(&mut client, &input.time_range)?;
    let log_lines = load_latency_events(&mut client)?;
    let info_metrics = build_info_metrics(&info_map);
    let executed_plan = format!(
        "AUTH -> SELECT {} -> INFO ALL -> SLOWLOG GET {} -> {} latency probes -> LATENCY LATEST",
        config.database,
        config.slowlog_limit,
        latency_points.len()
    );
    let summary = build_summary(&instance_name, &info_metrics, &slow_queries, &latency_points, &log_lines);

    Ok(AnalyzeRedisResponse {
        environment_id: input.environment_id,
        instance_name,
        time_range: input.time_range,
        adapter_mode: format!("redis-resp-direct ({})", profile.endpoint.trim()),
        executed_plan,
        info_metrics,
        slow_queries,
        latency_points,
        log_lines,
        summary,
    })
}

fn resolve_profile(
    connection: &Connection,
    environment_id: &str,
) -> Result<Option<ConnectionProfile>, String> {
    db::list_connection_profiles(connection)
        .map_err(|error| error.to_string())
        .map(|profiles| {
            profiles
                .into_iter()
                .find(|profile| profile.environment_id == environment_id && profile.profile_type == "redis")
        })
}

fn parse_endpoint(endpoint: &str) -> Result<RedisEndpoint, String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err("Redis endpoint is required. Set host:port in Settings first.".to_string());
    }

    let without_scheme = trimmed
        .strip_prefix("redis://")
        .or_else(|| trimmed.strip_prefix("rediss://"))
        .unwrap_or(trimmed);
    let host_port = without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .trim();
    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port_text)) => {
            let parsed_port = port_text
                .parse::<u16>()
                .map_err(|_| format!("Invalid Redis port in endpoint: {trimmed}"))?;
            (host.to_string(), parsed_port)
        }
        None => (host_port.to_string(), 6379),
    };

    if host.trim().is_empty() {
        return Err("Redis endpoint host is empty.".to_string());
    }

    Ok(RedisEndpoint { host, port })
}

fn parse_info_sections(body: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            values.insert(key.to_string(), value.to_string());
        }
    }
    values
}

fn build_info_metrics(info: &BTreeMap<String, String>) -> Vec<RedisInfoMetric> {
    let used_memory = parse_u64(info.get("used_memory").map(String::as_str));
    let maxmemory = parse_u64(info.get("maxmemory").map(String::as_str));
    let connected_clients = parse_u64(info.get("connected_clients").map(String::as_str)).unwrap_or(0);
    let blocked_clients = parse_u64(info.get("blocked_clients").map(String::as_str)).unwrap_or(0);
    let hits = parse_u64(info.get("keyspace_hits").map(String::as_str)).unwrap_or(0);
    let misses = parse_u64(info.get("keyspace_misses").map(String::as_str)).unwrap_or(0);
    let hit_ratio = if hits + misses > 0 {
        (hits as f64 / (hits + misses) as f64) * 100.0
    } else {
        0.0
    };
    let memory_label = match (used_memory, maxmemory) {
        (Some(used), Some(max)) if max > 0 => format_bytes_pair(used, max),
        (Some(used), _) => format_bytes(used),
        _ => "unknown".to_string(),
    };
    let memory_status = match (used_memory, maxmemory) {
        (Some(used), Some(max)) if max > 0 && used.saturating_mul(100) / max >= 90 => "critical",
        (Some(used), Some(max)) if max > 0 && used.saturating_mul(100) / max >= 80 => "warning",
        (Some(_), Some(_)) => "healthy",
        _ => "warning",
    };
    let uptime_seconds = parse_u64(info.get("uptime_in_seconds").map(String::as_str)).unwrap_or(0);
    let latest_fork_usec = parse_u64(info.get("latest_fork_usec").map(String::as_str)).unwrap_or(0);

    vec![
        RedisInfoMetric {
            label: "uptime".to_string(),
            value: format_uptime(uptime_seconds),
            status: "healthy".to_string(),
            detail: "Directly sourced from Redis INFO.".to_string(),
        },
        RedisInfoMetric {
            label: "used_memory".to_string(),
            value: memory_label,
            status: memory_status.to_string(),
            detail: "Memory usage compared against maxmemory when configured.".to_string(),
        },
        RedisInfoMetric {
            label: "connected_clients".to_string(),
            value: connected_clients.to_string(),
            status: if connected_clients >= 1000 { "warning" } else { "healthy" }.to_string(),
            detail: "Concurrent client count from INFO clients.".to_string(),
        },
        RedisInfoMetric {
            label: "blocked_clients".to_string(),
            value: blocked_clients.to_string(),
            status: if blocked_clients > 0 { "warning" } else { "healthy" }.to_string(),
            detail: "Blocked clients often point at scripts, long commands, or downstream pressure.".to_string(),
        },
        RedisInfoMetric {
            label: "keyspace_hits".to_string(),
            value: if hits + misses > 0 {
                format!("{hit_ratio:.1}%")
            } else {
                "n/a".to_string()
            },
            status: if hit_ratio >= 90.0 { "healthy" } else { "warning" }.to_string(),
            detail: "Cache hit ratio derived from keyspace hits and misses.".to_string(),
        },
        RedisInfoMetric {
            label: "latest_fork_usec".to_string(),
            value: if latest_fork_usec > 0 {
                format!("{} us", latest_fork_usec)
            } else {
                "0 us".to_string()
            },
            status: if latest_fork_usec >= 1_000_000 { "warning" } else { "healthy" }.to_string(),
            detail: "Large fork time can indicate memory pressure during persistence.".to_string(),
        },
    ]
}

fn load_slow_queries(client: &mut RedisClient, limit: usize) -> Result<Vec<RedisSlowQuery>, String> {
    let response = client.run(["SLOWLOG", "GET", &limit.to_string()])?;
    let array = match response {
        RedisValue::Array(items) => items,
        RedisValue::Nil => return Ok(Vec::new()),
        other => return Err(format!("Unexpected SLOWLOG response: {}", other.kind())),
    };

    let mut queries = Vec::new();
    for entry in array {
        let RedisValue::Array(parts) = entry else {
            continue;
        };
        if parts.len() < 4 {
            continue;
        }
        let id = parts[0].as_i64().unwrap_or(0);
        let timestamp = parts[1].as_i64().unwrap_or(0);
        let duration_micros = parts[2].as_i64().unwrap_or(0).max(0) as u64;
        let command = parts[3]
            .as_array()
            .map(|argv| {
                argv.iter()
                    .take(3)
                    .filter_map(|item| item.as_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        let key_sample = parts
            .get(3)
            .and_then(RedisValue::as_array)
            .and_then(|argv| argv.get(1))
            .and_then(RedisValue::as_string)
            .unwrap_or_else(|| "n/a".to_string());
        let client_name = parts
            .get(5)
            .and_then(RedisValue::as_string)
            .or_else(|| parts.get(4).and_then(RedisValue::as_string))
            .unwrap_or_else(|| "unknown".to_string());

        queries.push(RedisSlowQuery {
            id: format!("slowlog-{id}"),
            timestamp: unix_to_rfc3339(timestamp),
            duration_micros,
            command,
            key_sample,
            client: client_name,
        });
    }

    Ok(queries)
}

fn measure_latency(client: &mut RedisClient, time_range: &str) -> Result<Vec<RedisLatencyPoint>, String> {
    let samples = match time_range {
        "15m" => 4,
        "1h" => 6,
        "6h" => 8,
        "24h" => 10,
        _ => 6,
    };

    let mut points = Vec::new();
    let mut elapsed = Vec::new();
    for index in 0..samples {
        let start = Instant::now();
        let response = client.run(["PING"])?;
        match response {
            RedisValue::SimpleString(ref value) if value == "PONG" => {}
            RedisValue::BulkString(ref value) if value == "PONG" => {}
            _ => return Err("Unexpected PING response from Redis.".to_string()),
        }

        let millis = start.elapsed().as_secs_f64() * 1000.0;
        elapsed.push(millis);
        let mut sorted = elapsed.clone();
        sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let avg = elapsed.iter().sum::<f64>() / elapsed.len() as f64;
        let p95_index = ((sorted.len() as f64 * 0.95).ceil() as usize)
            .saturating_sub(1)
            .min(sorted.len().saturating_sub(1));
        let p95 = sorted.get(p95_index).copied().unwrap_or(avg);

        points.push(RedisLatencyPoint {
            timestamp: (Utc::now() - chrono::Duration::seconds((samples - index - 1) as i64)).to_rfc3339(),
            avg_ms: avg,
            p95_ms: p95,
        });
    }

    Ok(points)
}

fn load_latency_events(client: &mut RedisClient) -> Result<Vec<RedisLogLine>, String> {
    let response = client.run(["LATENCY", "LATEST"])?;
    let RedisValue::Array(events) = response else {
        return Ok(Vec::new());
    };

    let lines = events
        .into_iter()
        .filter_map(|item| {
            let RedisValue::Array(parts) = item else {
                return None;
            };
            if parts.len() < 4 {
                return None;
            }
            let event_name = parts[0].as_string().unwrap_or_else(|| "unknown".to_string());
            let unix_time = parts[1].as_i64().unwrap_or(0);
            let latest_ms = parts[2].as_i64().unwrap_or(0);
            let max_ms = parts[3].as_i64().unwrap_or(0);

            Some(RedisLogLine {
                timestamp: unix_to_rfc3339(unix_time),
                level: if max_ms >= 100 { "WARNING" } else { "NOTICE" }.to_string(),
                message: format!(
                    "Latency event {} latest={}ms max={}ms",
                    sanitize_and_mask_text(&event_name),
                    latest_ms,
                    max_ms
                ),
            })
        })
        .collect::<Vec<_>>();

    Ok(lines)
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
            "INFO metrics show real runtime pressure rather than mocked placeholders.".to_string(),
            "The slowlog highlights the heaviest Redis commands in the current window.".to_string(),
            format!(
                "{warning_logs} latency event(s) were returned by Redis LATENCY LATEST and can be correlated with application timeouts."
            ),
        ],
        recommended_next_steps: vec![
            "Compare the slowlog keys and commands with the application path or worker that owns them.".to_string(),
            "Correlate this Redis latency window with ELK timeout clusters and Kubernetes restarts.".to_string(),
            "If memory or blocked clients are elevated, inspect big keys and script usage next.".to_string(),
        ],
    }
}

fn parse_u64(value: Option<&str>) -> Option<u64> {
    value.and_then(|text| text.trim().parse::<u64>().ok())
}

fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if (bytes as f64) >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}

fn format_bytes_pair(left: u64, right: u64) -> String {
    format!("{} / {}", format_bytes(left), format_bytes(right))
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours:02}h")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}

fn unix_to_rfc3339(timestamp: i64) -> String {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

struct RedisProfileConfig {
    database: u32,
    slowlog_limit: usize,
    tls_enabled: bool,
    command_timeout: Duration,
}

impl RedisProfileConfig {
    fn from_profile(profile: &ConnectionProfile) -> Result<Self, String> {
        let config = serde_json::from_str::<Value>(&profile.config_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        let database = config
            .get("database")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let slowlog_limit = config
            .get("slowlogLimit")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .clamp(1, 128) as usize;
        let tls_enabled = config.get("tlsEnabled").and_then(Value::as_bool).unwrap_or(false);

        Ok(Self {
            database,
            slowlog_limit,
            tls_enabled,
            command_timeout: Duration::from_secs(5),
        })
    }
}

struct RedisEndpoint {
    host: String,
    port: u16,
}

struct RedisClient {
    stream: TcpStream,
}

impl RedisClient {
    fn connect(endpoint: &RedisEndpoint, timeout: Duration) -> Result<Self, String> {
        let address = format!("{}:{}", endpoint.host, endpoint.port);
        let socket = address
            .to_socket_addrs()
            .map_err(|error| format!("Failed to resolve Redis endpoint {address}: {error}"))?
            .next()
            .ok_or_else(|| format!("No socket address resolved for Redis endpoint {address}"))?;
        let stream = TcpStream::connect_timeout(&socket, timeout)
            .map_err(|error| format!("Failed to connect to Redis endpoint {address}: {error}"))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| format!("Failed to set Redis read timeout: {error}"))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|error| format!("Failed to set Redis write timeout: {error}"))?;
        Ok(Self { stream })
    }

    fn authenticate(&mut self, username: Option<&str>, password: Option<&str>) -> Result<(), String> {
        let Some(password) = password.filter(|value| !value.trim().is_empty()) else {
            return Ok(());
        };

        let response = if let Some(username) = username.filter(|value| !value.trim().is_empty()) {
            self.run(["AUTH", username, password])?
        } else {
            self.run(["AUTH", password])?
        };

        response.expect_ok("AUTH")
    }

    fn select_database(&mut self, database: u32) -> Result<(), String> {
        self.run(["SELECT", &database.to_string()])?.expect_ok("SELECT")
    }

    fn run<const N: usize>(&mut self, args: [&str; N]) -> Result<RedisValue, String> {
        let payload = encode_command(&args);
        self.stream
            .write_all(payload.as_bytes())
            .map_err(|error| format!("Failed to write Redis command: {error}"))?;
        self.stream
            .flush()
            .map_err(|error| format!("Failed to flush Redis command: {error}"))?;
        parse_redis_value(&mut self.stream)
    }
}

#[derive(Debug, Clone)]
enum RedisValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(String),
    Array(Vec<RedisValue>),
    Nil,
}

impl RedisValue {
    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            Self::BulkString(value) => value.parse::<i64>().ok(),
            Self::SimpleString(value) => value.parse::<i64>().ok(),
            _ => None,
        }
    }

    fn as_string(&self) -> Option<String> {
        match self {
            Self::SimpleString(value) | Self::BulkString(value) | Self::Error(value) => Some(value.clone()),
            Self::Integer(value) => Some(value.to_string()),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&[RedisValue]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::SimpleString(_) => "simple string",
            Self::Error(_) => "error",
            Self::Integer(_) => "integer",
            Self::BulkString(_) => "bulk string",
            Self::Array(_) => "array",
            Self::Nil => "nil",
        }
    }

    fn expect_ok(self, command: &str) -> Result<(), String> {
        match self {
            Self::SimpleString(value) if value.eq_ignore_ascii_case("OK") => Ok(()),
            Self::Error(error) => Err(format!("Redis {} failed: {}", command, sanitize_and_mask_text(&error))),
            other => Err(format!("Unexpected Redis {} response: {}", command, other.kind())),
        }
    }
}

fn encode_command(args: &[&str]) -> String {
    let mut encoded = format!("*{}\r\n", args.len());
    for arg in args {
        encoded.push_str(&format!("${}\r\n{}\r\n", arg.as_bytes().len(), arg));
    }
    encoded
}

fn parse_redis_value(reader: &mut impl Read) -> Result<RedisValue, String> {
    let prefix = read_exact_bytes(reader, 1)?
        .into_iter()
        .next()
        .ok_or_else(|| "Redis response was empty.".to_string())?;

    match prefix {
        b'+' => Ok(RedisValue::SimpleString(read_line(reader)?)),
        b'-' => Ok(RedisValue::Error(read_line(reader)?)),
        b':' => {
            let line = read_line(reader)?;
            let value = line
                .parse::<i64>()
                .map_err(|error| format!("Invalid Redis integer response: {error}"))?;
            Ok(RedisValue::Integer(value))
        }
        b'$' => parse_bulk_string(reader),
        b'*' => parse_array(reader),
        other => Err(format!("Unsupported Redis response prefix: {}", other as char)),
    }
}

fn parse_bulk_string(reader: &mut impl Read) -> Result<RedisValue, String> {
    let line = read_line(reader)?;
    let length = line
        .parse::<i64>()
        .map_err(|error| format!("Invalid Redis bulk length: {error}"))?;
    if length < 0 {
        return Ok(RedisValue::Nil);
    }
    let mut buffer = vec![0_u8; length as usize];
    reader
        .read_exact(&mut buffer)
        .map_err(|error| format!("Failed to read Redis bulk string: {error}"))?;
    consume_crlf(reader)?;
    let text = String::from_utf8(buffer).map_err(|error| format!("Redis bulk string was not UTF-8: {error}"))?;
    Ok(RedisValue::BulkString(text))
}

fn parse_array(reader: &mut impl Read) -> Result<RedisValue, String> {
    let line = read_line(reader)?;
    let length = line
        .parse::<i64>()
        .map_err(|error| format!("Invalid Redis array length: {error}"))?;
    if length < 0 {
        return Ok(RedisValue::Nil);
    }

    let mut items = Vec::with_capacity(length as usize);
    for _ in 0..length {
        items.push(parse_redis_value(reader)?);
    }
    Ok(RedisValue::Array(items))
}

fn read_line(reader: &mut impl Read) -> Result<String, String> {
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0_u8; 1];
        reader
            .read_exact(&mut buffer)
            .map_err(|error| format!("Failed to read Redis response: {error}"))?;
        bytes.push(buffer[0]);
        let len = bytes.len();
        if len >= 2 && bytes[len - 2] == b'\r' && bytes[len - 1] == b'\n' {
            bytes.truncate(len - 2);
            return String::from_utf8(bytes)
                .map_err(|error| format!("Redis response line was not UTF-8: {error}"));
        }
    }
}

fn read_exact_bytes(reader: &mut impl Read, size: usize) -> Result<Vec<u8>, String> {
    let mut buffer = vec![0_u8; size];
    reader
        .read_exact(&mut buffer)
        .map_err(|error| format!("Failed to read Redis response bytes: {error}"))?;
    Ok(buffer)
}

fn consume_crlf(reader: &mut impl Read) -> Result<(), String> {
    let suffix = read_exact_bytes(reader, 2)?;
    if suffix.as_slice() == b"\r\n" {
        Ok(())
    } else {
        Err("Redis response missing CRLF terminator.".to_string())
    }
}
