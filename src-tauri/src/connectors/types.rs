use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai::providers::ProviderKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    None,
    ApiKey,
    Oauth,
    Local,
}

impl AuthType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ApiKey => "api_key",
            Self::Oauth => "oauth",
            Self::Local => "local",
        }
    }

    pub fn uses_secret(self) -> bool {
        matches!(self, Self::ApiKey | Self::Oauth)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionSpec {
    pub id: String,
    pub description: String,
    /// Changes something outside FocusDeck — always needs user confirmation.
    pub mutating: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSpec {
    pub id: String,
    pub description: String,
}

/// How "Test connection" checks a connector. None of these call a paid or
/// authenticated API unless the user already stored credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum HealthCheck {
    /// GET request; `{setting}` placeholders are filled from settings.
    Http { url: String },
    /// The program is found on PATH (nothing is executed).
    Executable { program: String },
    /// A secret is stored in the keyring.
    Credentials,
    /// Delegates to the AI Router's provider connection test.
    AiProvider { provider: ProviderKind },
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    pub auth_type: AuthType,
    /// Keyring entry to use instead of `connector.<id>.secret`, so connectors
    /// can share a key with another feature (e.g. the AI Assistant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
    pub permissions: Vec<String>,
    pub actions: Vec<ActionSpec>,
    pub events: Vec<EventSpec>,
    pub settings_schema: Value,
    pub health_check: HealthCheck,
    pub docs_url: String,
}

impl ConnectorManifest {
    pub fn secret_key(&self) -> String {
        self.secret_ref
            .clone()
            .unwrap_or_else(|| format!("connector.{}.secret", self.id))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum ConnectorStatus {
    Unknown,
    Ok,
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorInfo {
    pub manifest: ConnectorManifest,
    pub enabled: bool,
    pub settings: Value,
    pub has_secret: bool,
    pub status: ConnectorStatus,
}
