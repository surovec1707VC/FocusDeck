//! Provider abstraction shared by Ollama and OpenAI-compatible endpoints.

pub mod ollama;
pub mod openai_compatible;

use std::pin::Pin;

use async_trait::async_trait;
use futures_util::{stream, Stream, StreamExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Ollama,
    OpenaiCompatible,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OpenaiCompatible => "openai_compatible",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)] // used by the skills executor (section 6)
pub struct ChatResponse {
    pub content: String,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamChunk {
    Text(String),
    Stop(Option<String>),
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{provider} returned HTTP {status}: {body}")]
    Api {
        provider: &'static str,
        status: u16,
        body: String,
    },
    #[error("{0}")]
    Config(String),
    #[error("unexpected response: {0}")]
    Parse(String),
    #[error("stream error: {0}")]
    Stream(String),
    #[error(transparent)]
    Secret(#[from] crate::secrets::SecretError),
}

pub type ChatStream = Pin<Box<dyn Stream<Item = Result<StreamChunk, AiError>> + Send>>;

#[async_trait]
#[allow(dead_code)] // `kind` and `chat` are used by the skills executor (section 6)
pub trait AiProvider: Send + Sync {
    fn kind(&self) -> ProviderKind;

    async fn stream_chat(&self, request: &ChatRequest) -> Result<ChatStream, AiError>;

    async fn list_models(&self) -> Result<Vec<ModelInfo>, AiError>;

    async fn test_connection(&self) -> Result<(), AiError>;

    /// Non-streaming chat, built on the stream so each provider implements one path.
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AiError> {
        let mut stream = self.stream_chat(request).await?;
        let mut content = String::new();
        let mut stop_reason = None;
        while let Some(chunk) = stream.next().await {
            match chunk? {
                StreamChunk::Text(t) => content.push_str(&t),
                StreamChunk::Stop(r) => stop_reason = r,
            }
        }
        Ok(ChatResponse { content, stop_reason })
    }
}

/// Turns a non-2xx response into `AiError::Api` with the body for context.
pub(crate) async fn check_status(
    provider: &'static str,
    resp: reqwest::Response,
) -> Result<reqwest::Response, AiError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    Err(AiError::Api {
        provider,
        status: status.as_u16(),
        body: body.chars().take(500).collect(),
    })
}

/// Splits a streaming body into newline-delimited lines (for NDJSON).
pub(crate) fn ndjson_lines(
    resp: reqwest::Response,
) -> impl Stream<Item = Result<String, AiError>> + Send {
    let bytes = resp.bytes_stream();
    stream::unfold(
        (Box::pin(bytes), Vec::<u8>::new(), false),
        |(mut bytes, mut buf, mut ended)| async move {
            loop {
                if let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    return Some((Ok(line), (bytes, buf, ended)));
                }
                if ended {
                    let rest = String::from_utf8_lossy(&buf).trim().to_string();
                    buf.clear();
                    return (!rest.is_empty()).then(|| (Ok(rest), (bytes, buf, ended)));
                }
                match bytes.next().await {
                    Some(Ok(chunk)) => buf.extend_from_slice(&chunk),
                    Some(Err(e)) => return Some((Err(e.into()), (bytes, buf, true))),
                    None => ended = true,
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Serves one HTTP response with `body` (sent in small pieces to exercise
    /// line/event reassembly) and returns the base URL.
    async fn mock_server(content_type: &'static str, body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 8192];
            let _ = sock.read(&mut buf).await;
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\nconnection: close\r\n\r\n"
            );
            sock.write_all(head.as_bytes()).await.unwrap();
            for piece in body.as_bytes().chunks(7) {
                sock.write_all(piece).await.unwrap();
                sock.flush().await.unwrap();
            }
        });
        format!("http://{addr}")
    }

    fn request() -> ChatRequest {
        ChatRequest {
            model: "m".into(),
            system: Some("sys".into()),
            messages: vec![ChatMessage { role: Role::User, content: "hi".into() }],
            max_tokens: 100,
        }
    }

    #[tokio::test]
    async fn ollama_ndjson_stream() {
        let url = mock_server(
            "application/x-ndjson",
            "{\"message\":{\"content\":\"Hel\"},\"done\":false}\n\
             {\"message\":{\"content\":\"lo\"},\"done\":false}\n\
             {\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\"}\n",
        )
        .await;
        let p = ollama::OllamaProvider::new(reqwest::Client::new(), url);
        let r = p.chat(&request()).await.unwrap();
        assert_eq!(r.content, "Hello");
        assert_eq!(r.stop_reason.as_deref(), Some("stop"));
    }

    #[tokio::test]
    async fn openai_sse_stream() {
        let url = mock_server(
            "text/event-stream",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hi \"}}]}\n\n\
             data: {\"choices\":[{\"delta\":{\"content\":\"there\"}}]}\n\n\
             data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
             data: [DONE]\n\n",
        )
        .await;
        let p = openai_compatible::OpenAiCompatibleProvider::new(reqwest::Client::new(), url, None);
        let r = p.chat(&request()).await.unwrap();
        assert_eq!(r.content, "Hi there");
        assert_eq!(r.stop_reason.as_deref(), Some("stop"));
    }
}
