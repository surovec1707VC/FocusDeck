//! Ollama (local, no API key). Streaming uses NDJSON from `/api/chat`.

use async_trait::async_trait;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;

use super::{
    check_status, ndjson_lines, AiError, AiProvider, ChatRequest, ChatStream, ModelInfo,
    ProviderKind, StreamChunk,
};

pub const DEFAULT_URL: &str = "http://localhost:11434";

pub struct OllamaProvider {
    http: reqwest::Client,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(http: reqwest::Client, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }
}

#[derive(Deserialize)]
struct ChatLine {
    #[serde(default)]
    message: Option<LineMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct LineMessage {
    #[serde(default)]
    content: String,
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    async fn stream_chat(&self, request: &ChatRequest) -> Result<ChatStream, AiError> {
        let mut messages = Vec::new();
        if let Some(system) = &request.system {
            messages.push(json!({ "role": "system", "content": system }));
        }
        for m in &request.messages {
            messages.push(json!({ "role": m.role, "content": m.content }));
        }
        let body = json!({ "model": request.model, "messages": messages, "stream": true });

        let resp = self
            .http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?;
        let resp = check_status("ollama", resp).await?;

        let stream = ndjson_lines(resp).filter_map(|line| async move {
            let line = match line {
                Ok(l) => l,
                Err(e) => return Some(Err(e)),
            };
            let parsed: ChatLine = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(e) => return Some(Err(AiError::Parse(e.to_string()))),
            };
            if let Some(err) = parsed.error {
                return Some(Err(AiError::Stream(err)));
            }
            if parsed.done {
                return Some(Ok(StreamChunk::Stop(parsed.done_reason)));
            }
            parsed
                .message
                .filter(|m| !m.content.is_empty())
                .map(|m| Ok(StreamChunk::Text(m.content)))
        });
        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, AiError> {
        #[derive(Deserialize)]
        struct Tags {
            models: Vec<Tag>,
        }
        #[derive(Deserialize)]
        struct Tag {
            name: String,
        }
        let resp = self.http.get(format!("{}/api/tags", self.base_url)).send().await?;
        let tags: Tags = check_status("ollama", resp).await?.json().await?;
        Ok(tags
            .models
            .into_iter()
            .map(|t| ModelInfo { id: t.name.clone(), name: t.name })
            .collect())
    }

    async fn test_connection(&self) -> Result<(), AiError> {
        let resp = self.http.get(format!("{}/api/version", self.base_url)).send().await?;
        check_status("ollama", resp).await?;
        Ok(())
    }
}
