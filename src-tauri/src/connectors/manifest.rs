//! Loading and validating `connectors/*.json`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::types::{AuthType, ConnectorManifest, HealthCheck};

#[derive(Debug, Clone, Serialize)]
pub struct LoadIssue {
    pub file: String,
    pub errors: Vec<String>,
}

fn is_kebab(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_snake(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn validate(m: &ConnectorManifest, path: &Path) -> Vec<String> {
    let mut errors = Vec::new();

    if !is_kebab(&m.id) {
        errors.push(format!("id `{}` must be lowercase kebab-case", m.id));
    }
    if path.file_stem().is_some_and(|stem| stem != m.id.as_str()) {
        errors.push(format!("id `{}` must match the file name", m.id));
    }
    for (field, value) in [("name", &m.name), ("description", &m.description), ("category", &m.category)] {
        if value.trim().is_empty() {
            errors.push(format!("`{field}` must not be empty"));
        }
    }
    if !m.docs_url.starts_with("https://") {
        errors.push("docs_url must be an https:// link".into());
    }
    if m.settings_schema.get("type").and_then(Value::as_str) != Some("object") {
        errors.push("settings_schema must have \"type\": \"object\"".into());
    }
    if m.secret_ref.is_some() && !m.auth_type.uses_secret() {
        errors.push("secret_ref is only valid for api_key or oauth connectors".into());
    }

    let mut seen = HashSet::new();
    for a in &m.actions {
        if !is_snake(&a.id) {
            errors.push(format!("action id `{}` must be snake_case", a.id));
        }
        if !seen.insert(format!("action:{}", a.id)) {
            errors.push(format!("action `{}` is listed twice", a.id));
        }
    }
    for e in &m.events {
        if !is_snake(&e.id) {
            errors.push(format!("event id `{}` must be snake_case", e.id));
        }
        if !seen.insert(format!("event:{}", e.id)) {
            errors.push(format!("event `{}` is listed twice", e.id));
        }
    }

    match &m.health_check {
        HealthCheck::Credentials if !m.auth_type.uses_secret() => {
            errors.push("credentials health check needs auth_type api_key or oauth".into())
        }
        HealthCheck::Http { url } if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with('{') => {
            errors.push("http health check url must start with http(s):// or a {setting}".into())
        }
        HealthCheck::Executable { program } if program.contains(['/', '\\', ' ']) => {
            errors.push("executable health check takes a bare program name".into())
        }
        _ => {}
    }
    if m.auth_type == AuthType::None && m.secret_ref.is_some() {
        errors.push("auth_type none cannot have a secret".into());
    }
    errors
}

pub struct Loaded {
    pub manifest: ConnectorManifest,
}

/// Loads every `*.json` in `dir`; unreadable or invalid files become issues.
pub fn load_dir(dir: &Path) -> (Vec<Loaded>, Vec<LoadIssue>) {
    let mut loaded = Vec::new();
    let mut issues = Vec::new();

    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect(),
        Err(e) => {
            issues.push(LoadIssue {
                file: dir.display().to_string(),
                errors: vec![format!("cannot read connectors directory: {e}")],
            });
            return (loaded, issues);
        }
    };
    paths.sort();

    for path in paths {
        let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<ConnectorManifest>(&t).map_err(|e| e.to_string()));
        match parsed {
            Ok(manifest) => {
                let errors = validate(&manifest, &path);
                if errors.is_empty() {
                    loaded.push(Loaded { manifest });
                } else {
                    issues.push(LoadIssue { file, errors });
                }
            }
            Err(e) => issues.push(LoadIssue { file, errors: vec![e] }),
        }
    }
    (loaded, issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bundled_manifests_load() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../connectors");
        let (loaded, issues) = load_dir(&dir);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(loaded.len(), 16);
    }

    #[test]
    fn rejects_inconsistent_manifest() {
        let m: ConnectorManifest = serde_json::from_value(json!({
            "id": "demo", "name": "Demo", "description": "d", "category": "c", "enabled": false,
            "auth_type": "local", "secret_ref": "x", "permissions": [],
            "actions": [{ "id": "Do-It", "description": "", "mutating": false }],
            "events": [], "settings_schema": { "type": "object" },
            "health_check": { "type": "credentials" }, "docs_url": "http://example.com"
        }))
        .unwrap();
        let errors = validate(&m, Path::new("demo.json"));
        assert_eq!(errors.len(), 4, "{errors:?}");
    }
}
