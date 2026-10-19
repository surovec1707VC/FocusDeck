//! Manifest validation, the tool catalog skills may reference, and a small
//! JSON-Schema subset checker (type / required / properties / items / enum).

use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

use super::loader::{SafetyLevel, SkillManifest};
use crate::ai::profiles;

pub struct ToolSpec {
    pub name: &'static str,
    /// Permission a skill must declare to use the tool; `None` for reads of
    /// FocusDeck's own data.
    pub permission: Option<&'static str>,
    /// Changes state outside the conversation — always needs user confirmation.
    pub mutating: bool,
}

const fn tool(name: &'static str, permission: Option<&'static str>, mutating: bool) -> ToolSpec {
    ToolSpec { name, permission, mutating }
}

pub const TOOLS: &[ToolSpec] = &[
    tool("project.list_files", Some("fs:read"), false),
    tool("project.read_file", Some("fs:read"), false),
    tool("project.search", Some("fs:read"), false),
    tool("git.status", Some("git:read"), false),
    tool("git.diff", Some("git:read"), false),
    tool("git.log", Some("git:read"), false),
    tool("clipboard.read", Some("clipboard:read"), false),
    tool("clipboard.write", Some("clipboard:write"), true),
    tool("notes.search", None, false),
    tool("notes.create", Some("notes:write"), true),
    tool("tasks.list", None, false),
    tool("tasks.create", Some("tasks:write"), true),
    tool("sessions.search", None, false),
    tool("modules.list", None, false),
    tool("widgets.list", None, false),
    tool("widgets.update", Some("widgets:write"), true),
    tool("automations.list", None, false),
    tool("automations.create", Some("automations:write"), true),
    tool("ai.complete", Some("ai:use"), false),
    tool("app.open", Some("app:open"), true),
    tool("notifications.send", Some("notifications:send"), true),
    tool("shell.run", Some("shell:execute"), true),
];

pub fn find_tool(name: &str) -> Option<&'static ToolSpec> {
    TOOLS.iter().find(|t| t.name == name)
}

pub const MAX_STEPS_LIMIT: u32 = 20;

pub struct Validation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn validate_manifest(m: &SkillManifest, path: &Path) -> Validation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if !is_valid_id(&m.id) {
        errors.push(format!("id `{}` must be lowercase kebab-case", m.id));
    }
    if path.file_stem().is_some_and(|stem| stem != m.id.as_str()) {
        errors.push(format!("id `{}` must match the file name", m.id));
    }
    for (field, value) in [
        ("name", &m.name),
        ("description", &m.description),
        ("category", &m.category),
        ("system_prompt", &m.system_prompt),
    ] {
        if value.trim().is_empty() {
            errors.push(format!("`{field}` must not be empty"));
        }
    }
    if m.max_steps == 0 || m.max_steps > MAX_STEPS_LIMIT {
        errors.push(format!("max_steps must be between 1 and {MAX_STEPS_LIMIT}"));
    }
    if profiles::get(&m.model_profile).is_none() {
        errors.push(format!("unknown model_profile `{}`", m.model_profile));
    }
    for (field, schema) in [("input_schema", &m.input_schema), ("output_schema", &m.output_schema)] {
        if schema.get("type").and_then(Value::as_str) != Some("object") {
            errors.push(format!("{field} must be a JSON schema with \"type\": \"object\""));
        }
    }

    let permissions: HashSet<&str> = m.required_permissions.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    for name in &m.allowed_tools {
        if !seen.insert(name.as_str()) {
            errors.push(format!("tool `{name}` is listed twice"));
            continue;
        }
        let Some(spec) = find_tool(name) else {
            errors.push(format!("unknown tool `{name}`"));
            continue;
        };
        if let Some(p) = spec.permission {
            if !permissions.contains(p) {
                errors.push(format!("tool `{name}` needs permission `{p}` in required_permissions"));
            }
        }
        if spec.mutating && m.safety_level == SafetyLevel::ReadOnly {
            errors.push(format!("read_only skill cannot use state-changing tool `{name}`"));
        }
    }

    if m.safety_level == SafetyLevel::Dangerous {
        warnings.push("dangerous skill: every step requires confirmation".into());
    }
    if m.safety_level != SafetyLevel::ReadOnly
        && !m.allowed_tools.iter().any(|t| find_tool(t).is_some_and(|s| s.mutating))
    {
        warnings.push(format!(
            "safety_level is {} but no state-changing tools are allowed; read_only would be enough",
            m.safety_level.as_str()
        ));
    }

    Validation { errors, warnings }
}

/// Validates `value` against a JSON-Schema subset; returns readable errors.
pub fn check_schema(schema: &Value, value: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    check_at(schema, value, "$", &mut errors);
    errors
}

fn check_at(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    if let Some(ty) = schema.get("type").and_then(Value::as_str) {
        let ok = match ty {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            "null" => value.is_null(),
            _ => true,
        };
        if !ok {
            errors.push(format!("{path}: expected {ty}"));
            return;
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.contains(value) {
            errors.push(format!("{path}: value is not one of the allowed options"));
        }
    }
    if let Some(obj) = value.as_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if !obj.contains_key(key) {
                    errors.push(format!("{path}: missing required field `{key}`"));
                }
            }
        }
        if let Some(props) = schema.get("properties").and_then(Value::as_object) {
            for (key, sub) in props {
                if let Some(v) = obj.get(key) {
                    check_at(sub, v, &format!("{path}.{key}"), errors);
                }
            }
        }
    }
    if let (Some(items), Some(arr)) = (schema.get("items"), value.as_array()) {
        for (i, v) in arr.iter().enumerate() {
            check_at(items, v, &format!("{path}[{i}]"), errors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::skills::loader::load_dir;
    use serde_json::json;

    #[test]
    fn bundled_manifests_are_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../skills");
        let (loaded, _) = load_dir(&dir);
        for l in loaded {
            let v = validate_manifest(&l.manifest, &l.path);
            assert!(v.errors.is_empty(), "{}: {:?}", l.manifest.id, v.errors);
        }
    }

    fn manifest() -> SkillManifest {
        serde_json::from_value(json!({
            "id": "demo", "name": "Demo", "description": "d", "category": "c", "enabled": true,
            "required_permissions": [], "allowed_tools": [], "system_prompt": "p",
            "input_schema": { "type": "object" }, "output_schema": { "type": "object" },
            "safety_level": "read_only", "max_steps": 1, "model_profile": "fast_assistant"
        }))
        .unwrap()
    }

    #[test]
    fn rejects_bad_manifests() {
        let path = Path::new("demo.json");
        assert!(validate_manifest(&manifest(), path).errors.is_empty());

        let mut m = manifest();
        m.allowed_tools = vec!["shell.run".into()];
        let errors = validate_manifest(&m, path).errors;
        assert!(errors.iter().any(|e| e.contains("needs permission `shell:execute`")));
        assert!(errors.iter().any(|e| e.contains("read_only skill cannot use")));

        let mut m = manifest();
        m.allowed_tools = vec!["made.up".into()];
        m.max_steps = 0;
        m.model_profile = "nope".into();
        assert_eq!(validate_manifest(&m, path).errors.len(), 3);

        assert!(!validate_manifest(&manifest(), Path::new("other.json")).errors.is_empty());
    }

    #[test]
    fn schema_checks() {
        let schema = json!({
            "type": "object",
            "required": ["a"],
            "properties": {
                "a": { "type": "string" },
                "b": { "type": "array", "items": { "type": "integer" } },
                "c": { "type": "string", "enum": ["x", "y"] }
            }
        });
        assert!(check_schema(&schema, &json!({ "a": "s", "b": [1, 2], "c": "x" })).is_empty());
        assert_eq!(check_schema(&schema, &json!({ "b": [1, "2"], "c": "z" })).len(), 3);
        assert_eq!(check_schema(&schema, &json!([])), vec!["$: expected object"]);
    }
}
