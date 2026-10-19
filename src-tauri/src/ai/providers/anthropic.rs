//! Anthropic Messages API over raw HTTP (there is no official Rust SDK).
//! Streaming: SSE events `content_block_delta` (text_delta) and
//! `message_delta` (stop_reason). Server-side refusal fallback is enabled.

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    check_status, AiError, AiProvider, ChatRequest, ChatStream, ModelInfo, ProviderKind,
    StreamChunk,
};

const BASE_URL: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";
/// Lets the API re-run a declined request on a fallback model (`fallbacks: "default"`).
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

pub struct AnthropicProvider {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(http: reqwest::Client, api_key: String) -> Self {
        Self::with_base_url(http, api_key, BASE_URL.to_string())
    }

    pub fn with_base_url(http: reqwest::Client, api_key: String, base_url: String) -> Self {
        Self { http, api_key, base_url }
    }

    fn request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    ContentBlockDelta { delta: Value },
    MessageDelta { delta: Value },
    Error { error: Value },
    #[serde(other)]
    Other,
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Anthropic
    }

    async fn stream_chat(&self, request: &ChatRequest) -> Result<ChatStream, AiError> {
        let mut body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "messages": request.messages,
            "stream": true,
            "fallbacks": "default",
        });
        if let Some(system) = &request.system {
            body["system"] = json!(system);
        }
        if let Some(effort) = &request.effort {
            body["output_config"] = json!({ "effort": effort });
        }

        let resp = self
            .request(self.http.post(format!("{}/messages", self.base_url)))
            .header("anthropic-beta", FALLBACK_BETA)
            .json(&body)
            .send()
            .await?;
        let resp = check_status("anthropic", resp).await?;

        let stream = resp.bytes_stream().eventsource().filter_map(|event| async move {
            let event = match event {
                Ok(e) => e,
                Err(e) => return Some(Err(AiError::Stream(e.to_string()))),
            };
            match serde_json::from_str::<Event>(&event.data) {
                // Thinking deltas and other block types are skipped; only visible text is streamed.
                Ok(Event::ContentBlockDelta { delta }) => {
                    if delta["type"] == "text_delta" {
                        delta["text"].as_str().map(|t| Ok(StreamChunk::Text(t.to_string())))
                    } else {
                        None
                    }
                }
                Ok(Event::MessageDelta { delta }) => delta["stop_reason"]
                    .as_str()
                    .map(|r| Ok(StreamChunk::Stop(Some(r.to_string())))),
                Ok(Event::Error { error }) => Some(Err(AiError::Stream(
                    error["message"].as_str().unwrap_or("unknown error").to_string(),
                ))),
                Ok(Event::Other) => None,
                Err(e) => Some(Err(AiError::Parse(e.to_string()))),
            }
        });
        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, AiError> {
        #[derive(Deserialize)]
        struct Models {
            data: Vec<Model>,
        }
        #[derive(Deserialize)]
        struct Model {
            id: String,
            display_name: String,
        }
        let resp = self
            .request(self.http.get(format!("{}/models?limit=100", self.base_url)))
            .send()
            .await?;
        let models: Models = check_status("anthropic", resp).await?.json().await?;
        Ok(models
            .data
            .into_iter()
            .map(|m| ModelInfo { id: m.id, name: m.display_name })
            .collect())
    }

    async fn test_connection(&self) -> Result<(), AiError> {
        self.list_models().await.map(|_| ())
    }
}
