use reqwest::header::CONTENT_TYPE;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct QwenConfig {
    pub base_url: String,
    pub base_path: String,
    pub app_key: String,
    pub app_secret: String,
    pub model: String,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
struct QwenRequest {
    app_key: String,
    app_secret: String,
    model: String,
    messages: Vec<QwenMessage>,
    temperature: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct QwenMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub struct QwenSuccess {
    pub content: String,
    pub raw_body: String,
}

#[derive(Debug, Error)]
pub enum QwenError {
    #[error("qwen request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("qwen returned HTTP {status}: {body_preview}")]
    HttpStatus {
        status: u16,
        body_preview: String,
    },
    #[error("qwen returned an unsupported response shape: {body_preview}")]
    UnsupportedShape {
        body_preview: String,
    },
}

pub async fn complete_chat(
    config: &QwenConfig,
    messages: Vec<QwenMessage>,
) -> Result<QwenSuccess, QwenError> {
    let client = reqwest::Client::new();
    let base = config.base_url.trim_end_matches('/');
    let path = normalize_base_path(&config.base_path);
    let response = client
        .post(format!("{base}{path}"))
        .header(CONTENT_TYPE, &config.content_type)
        .json(&QwenRequest {
            app_key: config.app_key.clone(),
            app_secret: config.app_secret.clone(),
            model: config.model.clone(),
            messages,
            temperature: 0.2,
        })
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(QwenError::HttpStatus {
            status: status.as_u16(),
            body_preview: preview(&body),
        });
    }

    let content = extract_content(&body).ok_or_else(|| QwenError::UnsupportedShape {
        body_preview: preview(&body),
    })?;

    Ok(QwenSuccess {
        content,
        raw_body: body,
    })
}

fn normalize_base_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "/chat/completions".to_string();
    }

    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn extract_content(body: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(body).ok()?;

    if let Some(content) = parsed
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(read_content_value)
    {
        return Some(content);
    }

    if let Some(content) = parsed
        .get("output")
        .and_then(|output| output.get("choices"))
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(read_content_value)
    {
        return Some(content);
    }

    if let Some(content) = parsed
        .get("output")
        .and_then(|output| output.get("text"))
        .and_then(Value::as_str)
    {
        return Some(content.to_string());
    }

    if let Some(content) = parsed
        .get("data")
        .and_then(|data| data.get("content"))
        .and_then(read_content_value)
    {
        return Some(content);
    }

    None
}

fn read_content_value(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value.as_array().map(|items| {
        items
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| item.get("content").and_then(Value::as_str).map(str::to_string))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }).filter(|joined| !joined.trim().is_empty())
}

fn preview(body: &str) -> String {
    let trimmed = body.trim();
    let preview = trimmed.chars().take(400).collect::<String>();
    if trimmed.chars().count() > 400 {
        format!("{preview}...")
    } else {
        preview
    }
}
