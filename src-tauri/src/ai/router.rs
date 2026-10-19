//! AI Router: builds the right provider for a request, injecting endpoint
//! config and API keys from the SecretStore.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::providers::{
    anthropic::AnthropicProvider, ollama, ollama::OllamaProvider, openai_compatible,
    openai_compatible::OpenAiCompatibleProvider, AiError, AiProvider, ProviderKind,
};
use crate::secrets::SecretStore;

const CONFIG_KEY: &str = "ai.router";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    pub ollama_url: String,
    pub openai_base_url: String,
    /// Model used for OpenAI-compatible requests when a profile doesn't target that provider.
    pub openai_default_model: String,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            ollama_url: ollama::DEFAULT_URL.to_string(),
            openai_base_url: openai_compatible::DEFAULT_BASE_URL.to_string(),
            openai_default_model: String::new(),
        }
    }
}

pub struct AiRouter {
    http: reqwest::Client,
    secrets: SecretStore,
    config: Mutex<RouterConfig>,
}

impl AiRouter {
    pub async fn load(pool: &SqlitePool, secrets: SecretStore) -> anyhow::Result<Self> {
        let stored: Option<(String,)> = sqlx::query_as("SELECT value_json FROM settings WHERE key = ?")
            .bind(CONFIG_KEY)
            .fetch_optional(pool)
            .await?;
        let config = stored
            .and_then(|(json,)| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Ok(Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("FocusDeck/", env!("CARGO_PKG_VERSION")))
                .build()?,
            secrets,
            config: Mutex::new(config),
        })
    }

    pub fn config(&self) -> RouterConfig {
        self.config.lock().unwrap().clone()
    }

    pub async fn set_config(&self, pool: &SqlitePool, config: RouterConfig) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value_json) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = datetime('now')",
        )
        .bind(CONFIG_KEY)
        .bind(serde_json::to_string(&config)?)
        .execute(pool)
        .await?;
        *self.config.lock().unwrap() = config;
        Ok(())
    }

    pub fn secrets(&self) -> &SecretStore {
        &self.secrets
    }

    pub fn provider(&self, kind: ProviderKind) -> Result<Box<dyn AiProvider>, AiError> {
        let config = self.config();
        let api_key = match kind.secret_key() {
            Some(key) => self.secrets.get(key)?,
            None => None,
        };
        Ok(match kind {
            ProviderKind::Ollama => Box::new(OllamaProvider::new(self.http.clone(), config.ollama_url)),
            ProviderKind::OpenaiCompatible => Box::new(OpenAiCompatibleProvider::new(
                self.http.clone(),
                config.openai_base_url,
                api_key,
            )),
            ProviderKind::Anthropic => Box::new(AnthropicProvider::new(
                self.http.clone(),
                api_key.ok_or(AiError::MissingApiKey("anthropic"))?,
            )),
        })
    }
}
