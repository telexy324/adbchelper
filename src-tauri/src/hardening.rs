use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

const SUSPICIOUS_PATTERNS: [&str; 6] = [
    "ignore previous instructions",
    "system prompt",
    "developer message",
    "<tool_call",
    "BEGIN PROMPT INJECTION",
    "assistant must",
];

pub fn sanitize_untrusted_text(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let lowered = line.to_ascii_lowercase();
            if SUSPICIOUS_PATTERNS
                .iter()
                .any(|pattern| lowered.contains(&pattern.to_ascii_lowercase()))
            {
                "[filtered potentially unsafe content]".to_string()
            } else {
                line.chars()
                    .filter(|char| !char.is_control() || matches!(char, '\n' | '\r' | '\t'))
                    .collect::<String>()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn mask_sensitive_text(input: &str) -> String {
    let mut masked = input.to_string();
    for marker in [
        "Bearer ",
        "accessToken=",
        "password=",
        "passwd=",
        "token=",
        "secret=",
        "apiKey=",
        "api_key=",
    ] {
        masked = mask_after_marker(&masked, marker);
    }
    masked
}

pub fn sanitize_and_mask_text(input: &str) -> String {
    mask_sensitive_text(&sanitize_untrusted_text(input))
}

pub fn sanitize_and_mask_json(input: &str) -> String {
    match serde_json::from_str::<Value>(input) {
        Ok(value) => serde_json::to_string(&sanitize_value(value)).unwrap_or_else(|_| sanitize_and_mask_text(input)),
        Err(_) => sanitize_and_mask_text(input),
    }
}

fn sanitize_value(value: Value) -> Value {
    match value {
        Value::String(string) => Value::String(sanitize_and_mask_text(&string)),
        Value::Array(items) => Value::Array(items.into_iter().map(sanitize_value).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let lowered = key.to_ascii_lowercase();
                    if ["password", "token", "secret", "apikey", "api_key", "accessToken"]
                        .iter()
                        .any(|needle| lowered.contains(&needle.to_ascii_lowercase()))
                    {
                        (key, Value::String("[masked]".to_string()))
                    } else {
                        (key, sanitize_value(value))
                    }
                })
                .collect(),
        ),
        other => other,
    }
}

pub fn run_command_with_timeout(
    command: &mut Command,
    timeout: Duration,
    label: &str,
) -> Result<Output, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to start {}: {}", label, error))?;
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|error| format!("Failed to capture {} output: {}", label, error));
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{} timed out after {}s.", label, timeout.as_secs()));
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Failed while waiting for {}: {}", label, error));
            }
        }
    }
}

fn mask_after_marker(input: &str, marker: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(marker) {
        let (before, after_marker_start) = remaining.split_at(index);
        result.push_str(before);
        result.push_str(marker);
        let after_marker = &after_marker_start[marker.len()..];
        let token_end = after_marker
            .find(|char: char| char.is_whitespace() || matches!(char, '"' | '\'' | ',' | ';'))
            .unwrap_or(after_marker.len());
        if token_end > 0 {
            result.push_str("[masked]");
            remaining = &after_marker[token_end..];
        } else {
            remaining = after_marker;
        }
    }

    result.push_str(remaining);
    result
}
