use std::collections::{BTreeMap, BTreeSet};

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde_json::Value;

use crate::models::connection_profile::ConnectionProfile;
use crate::hardening::{sanitize_and_mask_text, sanitize_untrusted_text};
use crate::models::nacos::{
    CompareNacosConfigInput, CompareNacosConfigResponse, NacosConfigVersion, NacosDiffEntry,
    NacosDiffSummary,
};
use crate::storage::secrets;

pub async fn compare_config(
    source_profile: ConnectionProfile,
    target_profile: ConnectionProfile,
    input: CompareNacosConfigInput,
) -> Result<CompareNacosConfigResponse, String> {
    let namespace_id = input
        .namespace_id
        .clone()
        .or_else(|| source_profile.default_scope.clone())
        .or_else(|| target_profile.default_scope.clone());

    let source = fetch_config(&source_profile, &input.data_id, &input.group, namespace_id.as_deref()).await?;
    let target = fetch_config(&target_profile, &input.data_id, &input.group, namespace_id.as_deref()).await?;
    let diff_entries = diff_config_values(&source.value, &target.value);
    let summary = build_summary(&input, &diff_entries);

    Ok(CompareNacosConfigResponse {
        source_environment_id: input.source_environment_id,
        target_environment_id: input.target_environment_id,
        data_id: input.data_id,
        group: input.group,
        namespace_id: namespace_id.clone(),
        adapter_mode: "nacos-http-compare".to_string(),
        source: NacosConfigVersion {
            environment_id: source_profile.environment_id.clone(),
            profile_name: source_profile.name.clone(),
            namespace_id: namespace_id.clone().or_else(|| {
                source_profile
                    .default_scope
                    .clone()
                    .filter(|value| !value.trim().is_empty())
            }),
            value: source.value,
        },
        target: NacosConfigVersion {
            environment_id: target_profile.environment_id.clone(),
            profile_name: target_profile.name.clone(),
            namespace_id: namespace_id.clone().or_else(|| {
                target_profile
                    .default_scope
                    .clone()
                    .filter(|value| !value.trim().is_empty())
            }),
            value: target.value,
        },
        diff_entries,
        summary,
    })
}

struct LoadedConfig {
    value: String,
}

async fn fetch_config(
    profile: &ConnectionProfile,
    data_id: &str,
    group: &str,
    namespace_id: Option<&str>,
) -> Result<LoadedConfig, String> {
    let config = parse_profile_config(profile)?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|error| format!("Failed to build Nacos HTTP client: {error}"))?;
    let mut request = client
        .get(config.resolve_url(&profile.endpoint))
        .query(&[
            (config.namespace_query_name(), namespace_id.unwrap_or(config.default_namespace())),
            ("group", group),
            ("dataId", data_id),
        ]);

    request = apply_auth(request, profile, &config).await?;

    let response = send_with_retry(request.header(CONTENT_TYPE, "application/x-www-form-urlencoded"), &profile.name).await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "".to_string());
        return Err(format!(
            "Nacos request failed for {} with status {}: {}",
            profile.name,
            status,
            sanitize_and_mask_text(body.trim())
        ));
    }

    if config.api_version == "v2" {
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| format!("Invalid Nacos v2 response for {}: {error}", profile.name))?;
        let value = body
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Nacos v2 response missing data field for {}", profile.name))?;
        Ok(LoadedConfig {
            value: sanitize_untrusted_text(value),
        })
    } else {
        let body = response
            .text()
            .await
            .map_err(|error| format!("Invalid Nacos v1 response for {}: {error}", profile.name))?;
        Ok(LoadedConfig {
            value: sanitize_untrusted_text(&body),
        })
    }
}

async fn apply_auth(
    request: reqwest::RequestBuilder,
    profile: &ConnectionProfile,
    config: &NacosProfileConfig,
) -> Result<reqwest::RequestBuilder, String> {
    let secret = if profile.has_secret {
        Some(secrets::get_profile_secret(&profile.id).map_err(|error| error.to_string())?)
    } else {
        None
    };

    match config.auth_mode.as_str() {
        "basic" => {
            let username = profile
                .username
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("Nacos profile {} requires a username for basic auth.", profile.name))?;
            let password = secret.unwrap_or_default();
            Ok(request.basic_auth(username, Some(password)))
        }
        "bearer" => {
            let token = secret.ok_or_else(|| format!("Nacos profile {} requires a bearer token secret.", profile.name))?;
            Ok(request.header(AUTHORIZATION, format!("Bearer {token}")))
        }
        "accessToken" => {
            let token = secret.ok_or_else(|| format!("Nacos profile {} requires an access token secret.", profile.name))?;
            Ok(request.query(&[("accessToken", token)]))
        }
        "none" => Ok(request),
        other => Err(format!("Unsupported Nacos authMode: {other}")),
    }
}

fn diff_config_values(source: &str, target: &str) -> Vec<NacosDiffEntry> {
    let source_map = flatten_value(parse_config_value(source));
    let target_map = flatten_value(parse_config_value(target));
    let keys = source_map
        .keys()
        .chain(target_map.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut entries = keys
        .into_iter()
        .filter_map(|key| {
            let source_value = source_map.get(&key).cloned();
            let target_value = target_map.get(&key).cloned();
            let status = match (&source_value, &target_value) {
                (Some(left), Some(right)) if left == right => "unchanged",
                (Some(_), Some(_)) => "changed",
                (Some(_), None) => "removed",
                (None, Some(_)) => "added",
                (None, None) => return None,
            };

            Some(NacosDiffEntry {
                key,
                status: status.to_string(),
                source_value,
                target_value,
            })
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| {
        status_rank(&left.status)
            .cmp(&status_rank(&right.status))
            .then_with(|| left.key.cmp(&right.key))
    });
    entries
}

fn build_summary(input: &CompareNacosConfigInput, diff_entries: &[NacosDiffEntry]) -> NacosDiffSummary {
    let changed = diff_entries.iter().filter(|entry| entry.status == "changed").count();
    let added = diff_entries.iter().filter(|entry| entry.status == "added").count();
    let removed = diff_entries.iter().filter(|entry| entry.status == "removed").count();
    let impactful = diff_entries
        .iter()
        .filter(|entry| is_high_risk_key(&entry.key))
        .map(|entry| format!("{} ({})", entry.key, entry.status))
        .collect::<Vec<_>>();

    let mut likely_impact = Vec::new();
    if impactful.is_empty() {
        likely_impact.push("No obviously high-risk config keys were detected in the diff.".to_string());
    } else {
        likely_impact.push(format!(
            "High-risk drift detected in {} key(s): {}.",
            impactful.len(),
            impactful.join(", ")
        ));
    }
    if changed > 0 {
        likely_impact.push("Changed values can alter runtime behavior without changing deployment artifacts.".to_string());
    }
    if added > 0 || removed > 0 {
        likely_impact.push("Added or removed keys can change fallback behavior and feature gates.".to_string());
    }

    let explanation = vec![
        format!(
            "{} vs {} for {}/{} produced {} diff entries.",
            input.source_environment_id,
            input.target_environment_id,
            input.group,
            input.data_id,
            diff_entries.len()
        ),
        "Prioritize keys related to datasource, redis, kafka, endpoint, timeout, thread pool, switch, or feature flags.".to_string(),
        "If prod differs from test, verify whether the drift is intentional before rollout or incident response.".to_string(),
    ];

    NacosDiffSummary {
        headline: format!(
            "{} changed, {} added, {} removed when comparing {} to {}.",
            changed, added, removed, input.source_environment_id, input.target_environment_id
        ),
        likely_impact,
        explanation,
    }
}

fn parse_config_value(value: &str) -> Value {
    if let Ok(json) = serde_json::from_str::<Value>(value) {
        return json;
    }

    let mut map = serde_json::Map::new();
    for line in value.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, raw_value)) = trimmed.split_once('=') {
            map.insert(key.trim().to_string(), Value::String(raw_value.trim().to_string()));
        } else if let Some((key, raw_value)) = trimmed.split_once(':') {
            map.insert(key.trim().to_string(), Value::String(raw_value.trim().to_string()));
        }
    }

    Value::Object(map)
}

fn flatten_value(value: Value) -> BTreeMap<String, String> {
    let mut flattened = BTreeMap::new();
    flatten_into("", &value, &mut flattened);
    flattened
}

fn flatten_into(prefix: &str, value: &Value, output: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                let next_prefix = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_into(&next_prefix, nested, output);
            }
        }
        Value::Array(items) => {
            for (index, nested) in items.iter().enumerate() {
                flatten_into(&format!("{prefix}[{index}]"), nested, output);
            }
        }
        Value::Null => {
            output.insert(prefix.to_string(), "null".to_string());
        }
        Value::Bool(boolean) => {
            output.insert(prefix.to_string(), boolean.to_string());
        }
        Value::Number(number) => {
            output.insert(prefix.to_string(), number.to_string());
        }
        Value::String(string) => {
            output.insert(prefix.to_string(), string.clone());
        }
    }
}

fn status_rank(status: &str) -> u8 {
    match status {
        "changed" => 0,
        "removed" => 1,
        "added" => 2,
        _ => 3,
    }
}

fn is_high_risk_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "datasource",
        "redis",
        "kafka",
        "timeout",
        "feature",
        "switch",
        "thread",
        "pool",
        "endpoint",
        "url",
        "password",
        "username",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

async fn send_with_retry(
    request: reqwest::RequestBuilder,
    profile_name: &str,
) -> Result<reqwest::Response, String> {
    let cloned = request
        .try_clone()
        .ok_or_else(|| format!("Failed to clone request for {}.", profile_name))?;

    match request.send().await {
        Ok(response) => Ok(response),
        Err(first_error) => cloned
            .send()
            .await
            .map_err(|second_error| format!("Nacos request failed for {}: {} / retry: {}", profile_name, first_error, second_error)),
    }
}

struct NacosProfileConfig {
    api_version: String,
    auth_mode: String,
    namespace_id: Option<String>,
}

impl NacosProfileConfig {
    fn resolve_url(&self, endpoint: &str) -> String {
        let base = endpoint.trim_end_matches('/');
        if self.api_version == "v2" {
            format!("{base}/nacos/v2/cs/config")
        } else {
            format!("{base}/nacos/v1/cs/configs")
        }
    }

    fn namespace_query_name(&self) -> &'static str {
        if self.api_version == "v2" {
            "namespaceId"
        } else {
            "tenant"
        }
    }

    fn default_namespace(&self) -> &str {
        self.namespace_id.as_deref().unwrap_or("public")
    }
}

fn parse_profile_config(profile: &ConnectionProfile) -> Result<NacosProfileConfig, String> {
    let config = serde_json::from_str::<Value>(&profile.config_json)
        .unwrap_or_else(|_| Value::Object(Default::default()));

    let api_version = config
        .get("apiVersion")
        .and_then(Value::as_str)
        .unwrap_or("v1")
        .to_string();
    let auth_mode = config
        .get("authMode")
        .and_then(Value::as_str)
        .unwrap_or(if profile.has_secret && profile.username.is_some() { "basic" } else { "none" })
        .to_string();
    let namespace_id = config
        .get("namespaceId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| profile.default_scope.clone().filter(|value| !value.trim().is_empty()));

    Ok(NacosProfileConfig {
        api_version,
        auth_mode,
        namespace_id,
    })
}
