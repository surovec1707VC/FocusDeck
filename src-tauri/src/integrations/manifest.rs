//! Connector manifests: one JSON file per integration in `manifests/`,
//! compiled into the binary and validated at startup (and in tests).
//! Format — `docs/integration.schema.json`.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

const SOURCES: &[&str] = &[
    include_str!("manifests/github.json"),
    include_str!("manifests/gitlab.json"),
    include_str!("manifests/vscode.json"),
    include_str!("manifests/windows_terminal.json"),
    include_str!("manifests/obs.json"),
    include_str!("manifests/twitch.json"),
    include_str!("manifests/discord.json"),
    include_str!("manifests/telegram.json"),
    include_str!("manifests/notion.json"),
    include_str!("manifests/google_calendar.json"),
    include_str!("manifests/spotify.json"),
    include_str!("manifests/browser.json"),
    include_str!("manifests/local_files.json"),
];

/// Where the connector talks to: this computer only, or a cloud service.
/// Cloud connectors are paused in Offline Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Local,
    Cloud,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Local => "local",
            Kind::Cloud => "cloud",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// No secret.
    None,
    /// Personal access token.
    Token,
    /// API key, bot token or webhook URL.
    ApiKey,
    /// OAuth 2 token (the sign-in flow comes later; a token can be saved by hand).
    Oauth2,
    /// Password of a local service (e.g. obs-websocket).
    Password,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Auth {
    #[serde(rename = "type")]
    pub kind: AuthType,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub hint: String,
    /// The connector works without the secret too.
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Read,
    Change,
    Dangerous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permission {
    pub id: String,
    pub name: String,
    pub description: String,
    pub risk: Risk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Connector permissions the action needs.
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// What the health check looks at. No check calls an external API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthKind {
    /// Always fine once enabled.
    None,
    /// A secret is saved (cloud: the API itself is not called yet).
    Token,
    /// A program is found: the path from `setting`, else `target` in PATH.
    Executable,
    /// The folder from `setting` exists.
    Path,
    /// Something listens on 127.0.0.1 at the port from `setting`.
    LocalPort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthSpec {
    pub kind: HealthKind,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub setting: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Icon key; the UI maps it to an icon.
    pub icon: String,
    pub version: String,
    pub kind: Kind,
    pub auth: Auth,
    pub permissions: Vec<Permission>,
    pub actions: Vec<Action>,
    pub events: Vec<Event>,
    /// Same JSON-Schema subset as module settings (`modules::schema`). Secrets never go here.
    pub settings: Value,
    pub health: HealthSpec,
    pub enabled_by_default: bool,
    pub docs_url: String,
}

impl Manifest {
    pub fn needs_secret(&self) -> bool {
        self.auth.kind != AuthType::None && !self.auth.optional
    }

    pub fn permission(&self, id: &str) -> Option<&Permission> {
        self.permissions.iter().find(|p| p.id == id)
    }
}

fn is_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

/// Checks what serde can't: ids, references between sections, the settings schema.
pub fn validate(m: &Manifest) -> Result<(), String> {
    let err = |what: String| Err(format!("манифест {}: {what}", m.id));
    if !is_id(&m.id) || m.id.contains('.') {
        return err("id: строчные латинские буквы, цифры и _".into());
    }
    if m.name.trim().is_empty() || m.description.trim().is_empty() || m.icon.trim().is_empty() {
        return err("нужны name, description и icon".into());
    }
    if !m.docs_url.starts_with("https://") {
        return err("docs_url должен начинаться с https://".into());
    }
    if m.auth.kind == AuthType::None && m.auth.optional {
        return err("optional имеет смысл только с auth.type ≠ none".into());
    }
    if m.kind == Kind::Cloud && m.auth.kind == AuthType::None {
        return err("облачному коннектору нужен auth".into());
    }
    let mut seen = HashSet::new();
    for p in &m.permissions {
        if !is_id(&p.id) || !seen.insert(p.id.as_str()) {
            return err(format!("permission «{}»: неверный или повторный id", p.id));
        }
    }
    let mut actions = HashSet::new();
    for a in &m.actions {
        if !is_id(&a.id) || !actions.insert(a.id.as_str()) {
            return err(format!("action «{}»: неверный или повторный id", a.id));
        }
        if let Some(p) = a.permissions.iter().find(|p| !seen.contains(p.as_str())) {
            return err(format!("action «{}» ссылается на неизвестное разрешение «{p}»", a.id));
        }
    }
    let mut events = HashSet::new();
    for e in &m.events {
        if !is_id(&e.id) || !events.insert(e.id.as_str()) {
            return err(format!("event «{}»: неверный или повторный id", e.id));
        }
    }
    let props = m.settings.get("properties").and_then(Value::as_object);
    if m.settings.get("type").and_then(Value::as_str) != Some("object") || props.is_none() {
        return err("settings: нужен объект { type: object, properties }".into());
    }
    let defaults = crate::modules::schema::defaults(&m.settings);
    crate::modules::schema::validate(&m.settings, &defaults).or_else(|e| err(format!("settings: значение по умолчанию не проходит схему: {e}")))?;
    for (k, p) in props.into_iter().flatten() {
        if p.get("default").is_none() {
            return err(format!("settings.{k}: нужен default"));
        }
        let lower = k.to_lowercase();
        if ["token", "secret", "password", "api_key"].iter().any(|s| lower.contains(s)) {
            return err(format!("settings.{k}: секреты хранятся в Credential Manager, не в настройках"));
        }
    }
    let setting_ok = |key: &Option<String>| key.as_deref().is_none_or(|k| props.is_some_and(|p| p.contains_key(k)));
    if !setting_ok(&m.health.setting) {
        return err("health.setting: нет такой настройки".into());
    }
    let need = |ok: bool, what: &str| if ok { Ok(()) } else { err(format!("health: {what}")) };
    match m.health.kind {
        HealthKind::Executable => need(m.health.target.is_some(), "для executable нужен target")?,
        HealthKind::Path | HealthKind::LocalPort => need(m.health.setting.is_some(), "нужен setting")?,
        HealthKind::Token => need(m.auth.kind != AuthType::None, "token без auth")?,
        HealthKind::None => {}
    }
    Ok(())
}

/// All manifests, parsed and validated once.
pub fn all() -> &'static [Manifest] {
    static ALL: OnceLock<Vec<Manifest>> = OnceLock::new();
    ALL.get_or_init(|| {
        let list: Vec<Manifest> = SOURCES
            .iter()
            .map(|s| {
                let m: Manifest = serde_json::from_str(s).unwrap_or_else(|e| panic!("манифест интеграции: {e}"));
                validate(&m).unwrap_or_else(|e| panic!("{e}"));
                m
            })
            .collect();
        let mut ids = HashSet::new();
        assert!(list.iter().all(|m| ids.insert(m.id.clone())), "повторный id манифеста");
        list
    })
}

pub fn get(id: &str) -> AppResult<&'static Manifest> {
    all().iter().find(|m| m.id == id).ok_or_else(|| AppError::NotFound(format!("интеграция {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_load_and_validate() {
        let all = all();
        assert_eq!(all.len(), 13);
        for id in ["github", "gitlab", "vscode", "windows_terminal", "obs", "twitch", "discord", "telegram", "notion", "google_calendar", "spotify", "browser", "local_files"] {
            let m = get(id).unwrap();
            assert!(!m.enabled_by_default, "{id} must be off by default");
        }
        assert!(get("nope").is_err());
        assert!(all.iter().filter(|m| m.kind == Kind::Cloud).all(|m| m.health.kind == HealthKind::Token));
    }

    #[test]
    fn validation_rejects_bad_manifests() {
        let base = || get("github").unwrap().clone();
        let mut m = base();
        m.actions[0].permissions = vec!["missing".into()];
        assert!(validate(&m).is_err());
        let mut m = base();
        m.settings = serde_json::json!({ "type": "object", "properties": { "api_token": { "type": "string", "default": "" } } });
        assert!(validate(&m).is_err());
        let mut m = base();
        m.auth.kind = AuthType::None;
        assert!(validate(&m).is_err());
        let mut m = base();
        m.docs_url = "http://example.com".into();
        assert!(validate(&m).is_err());
        let mut m = base();
        m.permissions.push(m.permissions[0].clone());
        assert!(validate(&m).is_err());
        assert!(serde_json::from_str::<Manifest>(r#"{"id":"x","unknown":1}"#).is_err());
    }
}
