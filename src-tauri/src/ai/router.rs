//! AI Router: builds the right provider for a request, injecting endpoint
//! config and API keys from the SecretStore, and decides which provider and
//! model a profile actually uses.

use std::collections::HashSet;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::profiles::AiProfile;
use super::providers::{
    anthropic, anthropic::AnthropicProvider, ollama, ollama::OllamaProvider, openai_compatible,
    openai_compatible::OpenAiCompatibleProvider, AiError, AiProvider, ProviderKind,
};
use crate::secrets::SecretStore;

const CONFIG_KEY: &str = "ai.router";
const ANTHROPIC_KEY: &str = "ai.anthropic.api_key";
/// Key of the original single OpenAI-compatible endpoint; kept so keys saved
/// before multi-endpoint support keep working for the `default` endpoint.
const LEGACY_OPENAI_KEY: &str = "ai.openai_compatible.api_key";
pub const DEFAULT_ENDPOINT_ID: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiEndpoint {
    pub id: String,
    pub name: String,
    pub base_url: String,
    /// Model used when this endpoint is picked without choosing a model.
    #[serde(default)]
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RouterConfig {
    pub ollama_url: String,
    pub ollama_default_model: String,
    pub openai_endpoints: Vec<OpenAiEndpoint>,
    /// Endpoint used whenever the provider is `openai_compatible`.
    pub active_openai_endpoint: String,
    /// When set, profiles use this provider instead of their own
    /// (except profiles pinned to a provider, like the local private one).
    pub default_provider: Option<ProviderKind>,
    // Pre-multi-endpoint fields, read once and migrated.
    #[serde(skip_serializing)]
    openai_base_url: Option<String>,
    #[serde(skip_serializing)]
    openai_default_model: Option<String>,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            ollama_url: ollama::DEFAULT_URL.to_string(),
            ollama_default_model: "llama3.2".into(),
            openai_endpoints: vec![OpenAiEndpoint {
                id: DEFAULT_ENDPOINT_ID.into(),
                name: "OpenAI".into(),
                base_url: openai_compatible::DEFAULT_BASE_URL.into(),
                default_model: String::new(),
            }],
            active_openai_endpoint: DEFAULT_ENDPOINT_ID.into(),
            default_provider: None,
            openai_base_url: None,
            openai_default_model: None,
        }
    }
}

impl RouterConfig {
    /// Moves legacy single-endpoint fields into the `default` endpoint.
    fn migrate(mut self) -> Self {
        let url = self.openai_base_url.take();
        let model = self.openai_default_model.take();
        if url.is_none() && model.is_none() {
            return self;
        }
        if !self.openai_endpoints.iter().any(|e| e.id == DEFAULT_ENDPOINT_ID) {
            self.openai_endpoints.insert(0, RouterConfig::default().openai_endpoints.remove(0));
        }
        let endpoint = self.openai_endpoints.iter_mut().find(|e| e.id == DEFAULT_ENDPOINT_ID).unwrap();
        if let Some(url) = url.filter(|u| !u.is_empty()) {
            endpoint.base_url = url;
        }
        if let Some(model) = model {
            endpoint.default_model = model;
        }
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.ollama_url.starts_with("http://") && !self.ollama_url.starts_with("https://") {
            return Err("Ollama URL must start with http:// or https://".into());
        }
        let mut ids = HashSet::new();
        for e in &self.openai_endpoints {
            let id_ok = !e.id.is_empty()
                && e.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !id_ok {
                return Err(format!("endpoint id `{}` must be lowercase letters, digits and dashes", e.id));
            }
            if !ids.insert(e.id.as_str()) {
                return Err(format!("endpoint id `{}` is used twice", e.id));
            }
            if e.name.trim().is_empty() {
                return Err(format!("endpoint `{}` needs a name", e.id));
            }
            if !e.base_url.starts_with("http://") && !e.base_url.starts_with("https://") {
                return Err(format!("{}: base URL must start with http:// or https://", e.name));
            }
        }
        let active_ok = if self.openai_endpoints.is_empty() {
            self.active_openai_endpoint.is_empty()
        } else {
            ids.contains(self.active_openai_endpoint.as_str())
        };
        if !active_ok {
            return Err("active OpenAI-compatible endpoint must be one of the endpoints".into());
        }
        if self.default_provider == Some(ProviderKind::OpenaiCompatible) && self.openai_endpoints.is_empty() {
            return Err("add an OpenAI-compatible endpoint before making it the default provider".into());
        }
        Ok(())
    }

    pub fn endpoint(&self, id: Option<&str>) -> Result<&OpenAiEndpoint, AiError> {
        let id = id.unwrap_or(&self.active_openai_endpoint);
        self.openai_endpoints
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| AiError::Config(format!("OpenAI-compatible endpoint `{id}` is not configured")))
    }
}

pub fn endpoint_secret_key(id: &str) -> String {
    if id == DEFAULT_ENDPOINT_ID {
        LEGACY_OPENAI_KEY.to_string()
    } else {
        format!("ai.openai_compatible.{id}.api_key")
    }
}

/// Which provider and model a request uses. Explicit choices win; otherwise a
/// pinned profile keeps its own provider, others follow `default_provider`.
pub fn resolve_target(
    config: &RouterConfig,
    profile: &AiProfile,
    provider: Option<ProviderKind>,
    model: Option<String>,
) -> Result<(ProviderKind, String), AiError> {
    let kind = provider.unwrap_or(if profile.pinned_provider {
        profile.provider
    } else {
        config.default_provider.unwrap_or(profile.provider)
    });
    if let Some(m) = model.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()) {
        return Ok((kind, m));
    }
    let model = if kind == profile.provider {
        profile.model.to_string()
    } else {
        match kind {
            ProviderKind::Anthropic => anthropic::DEFAULT_MODEL.to_string(),
            ProviderKind::Ollama => config.ollama_default_model.clone(),
            ProviderKind::OpenaiCompatible => config.endpoint(None)?.default_model.clone(),
        }
    };
    if model.trim().is_empty() {
        let place = match kind {
            ProviderKind::OpenaiCompatible => format!(
                "the `{}` endpoint",
                config.endpoint(None).map(|e| e.name.as_str()).unwrap_or("OpenAI-compatible")
            ),
            other => other.as_str().to_string(),
        };
        return Err(AiError::Config(format!(
            "choose a model, or set a default model for {place} in Providers & API keys"
        )));
    }
    Ok((kind, model))
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
            .and_then(|(json,)| serde_json::from_str::<RouterConfig>(&json).ok())
            .unwrap_or_default()
            .migrate();
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

    /// Validates and saves the config; API keys of removed endpoints are deleted.
    pub async fn set_config(&self, pool: &SqlitePool, config: RouterConfig) -> anyhow::Result<()> {
        config.validate().map_err(|e| anyhow::anyhow!(e))?;
        sqlx::query(
            "INSERT INTO settings (key, value_json) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = datetime('now')",
        )
        .bind(CONFIG_KEY)
        .bind(serde_json::to_string(&config)?)
        .execute(pool)
        .await?;
        let old = std::mem::replace(&mut *self.config.lock().unwrap(), config.clone());
        for removed in old.openai_endpoints.iter().filter(|o| !config.openai_endpoints.iter().any(|n| n.id == o.id)) {
            if let Err(e) = self.secrets.delete(&endpoint_secret_key(&removed.id)) {
                tauri_plugin_log::log::warn!("could not delete key of removed endpoint {}: {e}", removed.id);
            }
        }
        Ok(())
    }

    pub fn secrets(&self) -> &SecretStore {
        &self.secrets
    }

    /// Keyring entry for a provider's API key (`endpoint_id` only matters for
    /// OpenAI-compatible; `None` means the active endpoint).
    pub fn secret_key(&self, kind: ProviderKind, endpoint_id: Option<&str>) -> Result<Option<String>, AiError> {
        Ok(match kind {
            ProviderKind::Ollama => None,
            ProviderKind::Anthropic => Some(ANTHROPIC_KEY.to_string()),
            ProviderKind::OpenaiCompatible => Some(endpoint_secret_key(&self.config().endpoint(endpoint_id)?.id)),
        })
    }

    pub fn resolve(
        &self,
        profile: &AiProfile,
        provider: Option<ProviderKind>,
        model: Option<String>,
    ) -> Result<(ProviderKind, String), AiError> {
        resolve_target(&self.config(), profile, provider, model)
    }

    pub fn provider(&self, kind: ProviderKind) -> Result<Box<dyn AiProvider>, AiError> {
        self.provider_for(kind, None)
    }

    pub fn provider_for(&self, kind: ProviderKind, endpoint_id: Option<&str>) -> Result<Box<dyn AiProvider>, AiError> {
        let config = self.config();
        let api_key = match self.secret_key(kind, endpoint_id)? {
            Some(key) => self.secrets.get(&key)?,
            None => None,
        };
        Ok(match kind {
            ProviderKind::Ollama => Box::new(OllamaProvider::new(self.http.clone(), config.ollama_url)),
            ProviderKind::OpenaiCompatible => Box::new(OpenAiCompatibleProvider::new(
                self.http.clone(),
                config.endpoint(endpoint_id)?.base_url.clone(),
                api_key,
            )),
            ProviderKind::Anthropic => Box::new(AnthropicProvider::new(
                self.http.clone(),
                api_key.ok_or(AiError::MissingApiKey("anthropic"))?,
            )),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::profiles;

    fn endpoint(id: &str, model: &str) -> OpenAiEndpoint {
        OpenAiEndpoint { id: id.into(), name: id.into(), base_url: "https://x.test/v1".into(), default_model: model.into() }
    }

    #[test]
    fn migrates_legacy_single_endpoint() {
        let legacy = r#"{"ollama_url":"http://localhost:11434","openai_base_url":"https://api.groq.com/openai/v1","openai_default_model":"llama-x"}"#;
        let cfg = serde_json::from_str::<RouterConfig>(legacy).unwrap().migrate();
        assert_eq!(cfg.openai_endpoints.len(), 1);
        assert_eq!(cfg.openai_endpoints[0].id, DEFAULT_ENDPOINT_ID);
        assert_eq!(cfg.openai_endpoints[0].base_url, "https://api.groq.com/openai/v1");
        assert_eq!(cfg.openai_endpoints[0].default_model, "llama-x");
        assert!(!serde_json::to_string(&cfg).unwrap().contains("openai_base_url"));
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validation() {
        let mut cfg = RouterConfig::default();
        cfg.openai_endpoints.push(endpoint("default", ""));
        assert!(cfg.validate().unwrap_err().contains("twice"));

        let mut cfg = RouterConfig::default();
        cfg.active_openai_endpoint = "missing".into();
        assert!(cfg.validate().is_err());

        let mut cfg = RouterConfig::default();
        cfg.openai_endpoints.clear();
        cfg.active_openai_endpoint.clear();
        assert!(cfg.validate().is_ok());
        cfg.default_provider = Some(ProviderKind::OpenaiCompatible);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn secret_keys_keep_legacy_default() {
        assert_eq!(endpoint_secret_key("default"), "ai.openai_compatible.api_key");
        assert_eq!(endpoint_secret_key("groq"), "ai.openai_compatible.groq.api_key");
    }

    #[test]
    fn resolves_provider_and_model() {
        let fast = profiles::get("fast_assistant").unwrap();
        let local = profiles::get("local_private_assistant").unwrap();
        let mut cfg = RouterConfig::default();
        cfg.openai_endpoints.push(endpoint("groq", "llama-free"));
        cfg.active_openai_endpoint = "groq".into();

        // No default provider: the profile's own choice.
        assert_eq!(resolve_target(&cfg, &fast, None, None).unwrap(), (ProviderKind::Anthropic, fast.model.to_string()));

        // Default provider applies to normal profiles, with the endpoint's model…
        cfg.default_provider = Some(ProviderKind::OpenaiCompatible);
        assert_eq!(
            resolve_target(&cfg, &fast, None, None).unwrap(),
            (ProviderKind::OpenaiCompatible, "llama-free".into())
        );
        // …but not to the pinned local profile.
        assert_eq!(resolve_target(&cfg, &local, None, None).unwrap().0, ProviderKind::Ollama);

        // Explicit choices win.
        assert_eq!(
            resolve_target(&cfg, &fast, Some(ProviderKind::Ollama), Some("qwen".into())).unwrap(),
            (ProviderKind::Ollama, "qwen".into())
        );

        // Missing model gives a clear error.
        cfg.openai_endpoints[1].default_model.clear();
        assert!(resolve_target(&cfg, &fast, None, None).is_err());
    }
}
