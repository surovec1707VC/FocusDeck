//! Any endpoint speaking the OpenAI `/chat/completions` protocol
//! (LM Studio, vLLM, OpenRouter, llama.cpp server, ...). SSE streaming.

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;

use super::{
    check_status, AiError, AiProvider, ChatRequest, ChatStream, ModelInfo, ProviderKind,
    StreamChunk,
};

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAiCompatibleProvider {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl OpenAiCompatibleProvider {
    pub fn new(http: reqwest::Client, base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
        }
    }

    fn request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => builder.bearer_auth(key),
            None => builder,
        }
    }
}

#[derive(Deserialize)]
struct Chunk {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    #[serde(default)]
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

#[async_trait]
impl AiProvider for OpenAiCompatibleProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenaiCompatible
    }

    async fn stream_chat(&self, request: &ChatRequest) -> Result<ChatStream, AiError> {
        let mut messages = Vec::new();
        if let Some(system) = &request.system {
            messages.push(json!({ "role": "system", "content": system }));
        }
        for m in &request.messages {
            messages.push(json!({ "role": m.role, "content": m.content }));
        }
        let body = json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens,
            "stream": true,
        });

        let resp = self
            .request(self.http.post(format!("{}/chat/completions", self.base_url)))
            .json(&body)
            .send()
            .await?;
        let resp = check_status("openai_compatible", resp).await?;

        let stream = resp.bytes_stream().eventsource().filter_map(|event| async move {
            let event = match event {
                Ok(e) => e,
                Err(e) => return Some(Err(AiError::Stream(e.to_string()))),
            };
            if event.data.trim() == "[DONE]" {
                return None;
            }
            let chunk: Chunk = match serde_json::from_str(&event.data) {
                Ok(c) => c,
                Err(e) => return Some(Err(AiError::Parse(e.to_string()))),
            };
            let choice = chunk.choices.into_iter().next()?;
            if let Some(reason) = choice.finish_reason {
                return Some(Ok(StreamChunk::Stop(Some(reason))));
            }
            choice
                .delta
                .content
                .filter(|c| !c.is_empty())
                .map(|c| Ok(StreamChunk::Text(c)))
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
        }
        let resp = self
            .request(self.http.get(format!("{}/models", self.base_url)))
            .send()
            .await?;
        let models: Models = check_status("openai_compatible", resp).await?.json().await?;
        Ok(models
            .data
            .into_iter()
            .map(|m| ModelInfo { id: m.id.clone(), name: m.id })
            .collect())
    }

    async fn test_connection(&self) -> Result<(), AiError> {
        self.list_models().await.map(|_| ())
    }
}
