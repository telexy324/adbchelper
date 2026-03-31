use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct QwenConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Serialize)]
struct QwenRequest {
    model: String,
    messages: Vec<QwenMessage>,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QwenMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct QwenResponse {
    choices: Vec<QwenChoice>,
}

#[derive(Debug, Deserialize)]
struct QwenChoice {
    message: QwenMessage,
}

#[derive(Debug, Error)]
pub enum QwenError {
    #[error("qwen request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("qwen returned no assistant choice")]
    MissingChoice,
}

pub async fn complete_chat(
    config: &QwenConfig,
    messages: Vec<QwenMessage>,
) -> Result<String, QwenError> {
    let client = reqwest::Client::new();
    let base = config.base_url.trim_end_matches('/');
    let response = client
        .post(format!("{base}/chat/completions"))
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", config.api_key))
        .json(&QwenRequest {
            model: config.model.clone(),
            messages,
            temperature: 0.2,
        })
        .send()
        .await?
        .error_for_status()?
        .json::<QwenResponse>()
        .await?;

    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or(QwenError::MissingChoice)
}
