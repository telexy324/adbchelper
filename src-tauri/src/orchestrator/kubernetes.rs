use std::path::Path;
use std::process::Command;

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::models::connection_profile::ConnectionProfile;
use crate::models::kubernetes::{
    KubernetesEvent, KubernetesEventsSummary, ListKubernetesEventsInput, ListKubernetesEventsResponse,
};
use crate::storage::db;

pub fn list_events(
    connection: &Connection,
    input: ListKubernetesEventsInput,
) -> Result<ListKubernetesEventsResponse, String> {
    let profile = resolve_kubernetes_profile(connection, &input.environment_id)?;
    let config = KubernetesProfileConfig::from_profile(&profile)?;
    let events = fetch_events(&config, &input)?;
    let summary = summarize_events(&input, &events);

    Ok(ListKubernetesEventsResponse {
        environment_id: input.environment_id,
        namespace: input.namespace.clone(),
        adapter_mode: format!("kubectl-profile ({})", profile.name),
        query_summary: format!(
            "namespace={}, involvedObject={}, reason={}",
            input.namespace,
            input.involved_object.clone().unwrap_or_else(|| "*".to_string()),
            input.reason.clone().unwrap_or_else(|| "*".to_string())
        ),
        events,
        summary,
    })
}

fn resolve_kubernetes_profile(connection: &Connection, environment_id: &str) -> Result<ConnectionProfile, String> {
    db::list_connection_profiles(connection)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|profile| profile.environment_id == environment_id && profile.profile_type == "kubernetes")
        .ok_or_else(|| format!("No Kubernetes profile found for environment {environment_id}. Add one in Settings first."))
}

fn fetch_events(
    config: &KubernetesProfileConfig,
    input: &ListKubernetesEventsInput,
) -> Result<Vec<KubernetesEvent>, String> {
    let mut command = Command::new("kubectl");
    if let Some(kubeconfig_path) = &config.kubeconfig_path {
        if !Path::new(kubeconfig_path).exists() {
            return Err(format!("Configured kubeconfigPath does not exist: {kubeconfig_path}"));
        }
        command.arg("--kubeconfig").arg(kubeconfig_path);
    }
    if let Some(context) = &config.context {
        command.arg("--context").arg(context);
    }

    command
        .arg("get")
        .arg("events")
        .arg("-n")
        .arg(&input.namespace)
        .arg("--sort-by=.lastTimestamp")
        .arg("-o")
        .arg("json");

    let output = command
        .output()
        .map_err(|error| format!("Failed to execute kubectl: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("kubectl get events failed with status {}.", output.status)
        } else {
            format!("kubectl get events failed: {stderr}")
        });
    }

    let parsed = serde_json::from_slice::<KubectlEventsDocument>(&output.stdout)
        .map_err(|error| format!("Failed to parse kubectl events JSON: {error}"))?;
    let involved_filter = input
        .involved_object
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let reason_filter = input
        .reason
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    Ok(parsed
        .items
        .into_iter()
        .filter_map(|item| {
            let name = item.involved_object.name.unwrap_or_else(|| "unknown".to_string());
            let kind = item.involved_object.kind.unwrap_or_else(|| "Unknown".to_string());
            let reason = item.reason.unwrap_or_else(|| "Unknown".to_string());
            let message = item.message.unwrap_or_default();
            let timestamp = item
                .event_time
                .or(item.last_timestamp)
                .or(item.metadata.creation_timestamp)
                .unwrap_or_else(|| "unknown".to_string());

            if involved_filter
                .as_ref()
                .map(|filter| name.to_ascii_lowercase().contains(filter) || kind.to_ascii_lowercase().contains(filter))
                .unwrap_or(false)
                == false
                && involved_filter.is_some()
            {
                return None;
            }
            if reason_filter
                .as_ref()
                .map(|filter| reason.to_ascii_lowercase().contains(filter))
                .unwrap_or(false)
                == false
                && reason_filter.is_some()
            {
                return None;
            }

            Some(KubernetesEvent {
                id: item.metadata.uid.unwrap_or_else(|| Uuid::new_v4().to_string()),
                namespace: item.metadata.namespace.unwrap_or_else(|| input.namespace.clone()),
                kind,
                name,
                reason: reason.clone(),
                level: event_level(&item.r#type, &reason, &message),
                message,
                event_time: timestamp,
            })
        })
        .collect())
}

fn summarize_events(input: &ListKubernetesEventsInput, events: &[KubernetesEvent]) -> KubernetesEventsSummary {
    let warning_count = events.iter().filter(|event| event.level == "Warning").count();
    let top_reasons = events
        .iter()
        .take(3)
        .map(|event| format!("{} on {}", event.reason, event.name))
        .collect::<Vec<_>>();

    KubernetesEventsSummary {
        headline: format!(
            "{} event(s) in namespace {} with {} warning event(s).",
            events.len(),
            input.namespace,
            warning_count
        ),
        likely_impact: if top_reasons.is_empty() {
            vec!["No Kubernetes events matched the current filters.".to_string()]
        } else {
            vec![
                format!("Recent signals: {}.", top_reasons.join(", ")),
                "Warnings often align with rollout issues, image pulls, scheduling pressure, or probe failures.".to_string(),
            ]
        },
        recommended_next_steps: vec![
            "Correlate the hottest warning with log spikes and restart behavior in the same time window.".to_string(),
            "Attach this event set into the active investigation to compare against Nacos drift and host diagnostics.".to_string(),
        ],
    }
}

fn event_level(event_type: &Option<String>, reason: &str, message: &str) -> String {
    if event_type.as_deref() == Some("Warning")
        || reason.to_ascii_lowercase().contains("fail")
        || message.to_ascii_lowercase().contains("error")
    {
        "Warning".to_string()
    } else {
        "Normal".to_string()
    }
}

struct KubernetesProfileConfig {
    kubeconfig_path: Option<String>,
    context: Option<String>,
}

impl KubernetesProfileConfig {
    fn from_profile(profile: &ConnectionProfile) -> Result<Self, String> {
        let config = serde_json::from_str::<Value>(&profile.config_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        let kubeconfig_path = config
            .get("kubeconfigPath")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let context = config
            .get("context")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        if kubeconfig_path.is_none() && profile.endpoint.trim().is_empty() {
            return Err("Kubernetes profile requires kubeconfigPath in Extra JSON for local kubectl execution.".to_string());
        }

        Ok(Self {
            kubeconfig_path,
            context,
        })
    }
}

#[derive(Debug, Deserialize)]
struct KubectlEventsDocument {
    #[serde(default)]
    items: Vec<KubectlEventItem>,
}

#[derive(Debug, Deserialize)]
struct KubectlEventItem {
    metadata: KubectlMetadata,
    #[serde(rename = "type")]
    r#type: Option<String>,
    reason: Option<String>,
    message: Option<String>,
    #[serde(rename = "eventTime")]
    event_time: Option<String>,
    #[serde(rename = "lastTimestamp")]
    last_timestamp: Option<String>,
    #[serde(rename = "involvedObject")]
    involved_object: KubectlInvolvedObject,
}

#[derive(Debug, Deserialize)]
struct KubectlMetadata {
    uid: Option<String>,
    namespace: Option<String>,
    #[serde(rename = "creationTimestamp")]
    creation_timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KubectlInvolvedObject {
    kind: Option<String>,
    name: Option<String>,
}
