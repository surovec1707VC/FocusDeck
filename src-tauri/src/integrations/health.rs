//! Health checks, stage 1: only local facts (is it enabled and configured,
//! is a secret saved, is a program installed, does a folder exist, does a
//! local port answer). No check calls a cloud API.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::manifest::{HealthKind, Kind, Manifest};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Warning,
    Error,
    /// A secret or a required setting is missing.
    NotConfigured,
    /// Cloud connector while Offline Mode is on.
    Offline,
    Disabled,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warning => "warning",
            Status::Error => "error",
            Status::NotConfigured => "not_configured",
            Status::Offline => "offline",
            Status::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Health {
    pub status: Status,
    pub message: String,
}

fn h(status: Status, message: impl Into<String>) -> Health {
    Health { status, message: message.into() }
}

pub struct Input<'a> {
    pub manifest: &'a Manifest,
    pub settings: &'a Value,
    pub enabled: bool,
    pub offline: bool,
    pub has_secret: bool,
}

fn setting_str<'a>(settings: &'a Value, key: Option<&str>) -> &'a str {
    key.and_then(|k| settings.get(k)).and_then(Value::as_str).map(str::trim).unwrap_or("")
}

pub async fn check(i: Input<'_>) -> Health {
    let m = i.manifest;
    if !i.enabled {
        return h(Status::Disabled, "интеграция выключена");
    }
    if m.kind == Kind::Cloud && i.offline {
        return h(Status::Offline, "Offline Mode: облачные интеграции приостановлены");
    }
    if m.needs_secret() && !i.has_secret {
        return h(Status::NotConfigured, format!("нет секрета: {}", if m.auth.label.is_empty() { "токен" } else { &m.auth.label }));
    }
    let spec = &m.health;
    match spec.kind {
        HealthKind::None => h(Status::Ok, "готово"),
        HealthKind::Token => h(Status::Ok, "секрет сохранён в Credential Manager; запрос к API на этом этапе не выполняется"),
        HealthKind::Executable => {
            let custom = setting_str(i.settings, spec.setting.as_deref());
            if !custom.is_empty() {
                return if Path::new(custom).is_file() { h(Status::Ok, format!("найден {custom}")) } else { h(Status::Error, format!("файл не найден: {custom}")) };
            }
            let target = spec.target.as_deref().unwrap_or_default();
            match crate::devtools::which(target) {
                Some(p) => h(Status::Ok, format!("найден {}", p.display())),
                None => h(Status::Error, format!("«{target}» не найден в PATH — программа не установлена или путь не задан")),
            }
        }
        HealthKind::Path => {
            let dir = setting_str(i.settings, spec.setting.as_deref());
            if dir.is_empty() {
                h(Status::NotConfigured, "папка не выбрана")
            } else if !Path::new(dir).is_absolute() {
                h(Status::Error, "нужен полный путь к папке")
            } else if Path::new(dir).is_dir() {
                h(Status::Ok, format!("папка доступна: {dir}"))
            } else {
                h(Status::Error, format!("папка не найдена: {dir}"))
            }
        }
        HealthKind::LocalPort => {
            let port = spec.setting.as_deref().and_then(|k| i.settings.get(k)).and_then(Value::as_u64).unwrap_or(0);
            let Ok(port) = u16::try_from(port) else { return h(Status::Error, "неверный порт") };
            let probe = tokio::time::timeout(Duration::from_millis(800), tokio::net::TcpStream::connect(("127.0.0.1", port))).await;
            let note = if m.auth.kind != super::manifest::AuthType::None && !i.has_secret { " (пароль не сохранён)" } else { "" };
            match probe {
                Ok(Ok(_)) => h(Status::Ok, format!("127.0.0.1:{port} отвечает{note}")),
                _ => h(Status::Warning, format!("127.0.0.1:{port} не отвечает — программа не запущена или сервер выключен{note}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::manifest::get;
    use serde_json::json;

    async fn run(id: &str, settings: Value, enabled: bool, offline: bool, has_secret: bool) -> Health {
        check(Input { manifest: get(id).unwrap(), settings: &settings, enabled, offline, has_secret }).await
    }

    #[tokio::test]
    async fn statuses() {
        assert_eq!(run("github", json!({}), false, false, true).await.status, Status::Disabled);
        assert_eq!(run("github", json!({}), true, true, true).await.status, Status::Offline);
        assert_eq!(run("github", json!({}), true, false, false).await.status, Status::NotConfigured);
        assert_eq!(run("github", json!({}), true, false, true).await.status, Status::Ok);
        // Local connectors ignore Offline Mode.
        assert_eq!(run("browser", json!({}), true, true, false).await.status, Status::Ok);

        let dir = std::env::temp_dir();
        assert_eq!(run("local_files", json!({ "root": "" }), true, true, false).await.status, Status::NotConfigured);
        assert_eq!(run("local_files", json!({ "root": "relative" }), true, true, false).await.status, Status::Error);
        assert_eq!(run("local_files", json!({ "root": dir.to_string_lossy() }), true, true, false).await.status, Status::Ok);
        assert_eq!(run("vscode", json!({ "executable": dir.join("no-such-code.exe").to_string_lossy() }), true, false, false).await.status, Status::Error);

        // OBS: password optional; a free local port → warning, a listening one → ok.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let ok = run("obs", json!({ "port": port }), true, false, false).await;
        assert_eq!(ok.status, Status::Ok);
        assert!(ok.message.contains("пароль не сохранён"));
        drop(listener);
        assert_eq!(run("obs", json!({ "port": port }), true, false, true).await.status, Status::Warning);
    }
}
