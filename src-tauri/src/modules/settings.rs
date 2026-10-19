//! Settings helpers driven by a module's `settings_schema`.

use serde_json::{Map, Value};

fn properties(schema: &Value) -> Option<&Map<String, Value>> {
    schema.get("properties").and_then(Value::as_object)
}

/// Builds the default settings object from each property's `default`.
pub fn defaults(schema: &Value) -> Value {
    let mut out = Map::new();
    if let Some(props) = properties(schema) {
        for (key, prop) in props {
            if let Some(default) = prop.get("default") {
                out.insert(key.clone(), default.clone());
            }
        }
    }
    Value::Object(out)
}

/// Overlays `stored` on top of schema defaults, so new settings added in a
/// later version get their default value.
pub fn merge_with_defaults(schema: &Value, stored: &Value) -> Value {
    let mut merged = defaults(schema);
    if let (Some(target), Some(source)) = (merged.as_object_mut(), stored.as_object()) {
        for (k, v) in source {
            target.insert(k.clone(), v.clone());
        }
    }
    merged
}

/// Checks that `settings` is an object, has no unknown keys and that each
/// value matches the declared `type` (and `enum`, when given).
pub fn validate(schema: &Value, settings: &Value) -> Result<(), String> {
    let obj = settings
        .as_object()
        .ok_or_else(|| "settings must be an object".to_string())?;
    let Some(props) = properties(schema) else {
        return if obj.is_empty() {
            Ok(())
        } else {
            Err("module has no settings".into())
        };
    };

    for (key, value) in obj {
        let prop = props
            .get(key)
            .ok_or_else(|| format!("unknown setting `{key}`"))?;
        if let Some(ty) = prop.get("type").and_then(Value::as_str) {
            let ok = match ty {
                "string" => value.is_string(),
                "boolean" => value.is_boolean(),
                "integer" => value.is_i64() || value.is_u64(),
                "number" => value.is_number(),
                "array" => value.is_array(),
                "object" => value.is_object(),
                _ => true,
            };
            if !ok {
                return Err(format!("setting `{key}` must be of type {ty}"));
            }
        }
        if let Some(allowed) = prop.get("enum").and_then(Value::as_array) {
            if !allowed.contains(value) {
                return Err(format!("setting `{key}` has a value outside the allowed list"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({ "properties": {
            "limit": { "type": "integer", "default": 50 },
            "mode": { "type": "string", "enum": ["a", "b"], "default": "a" }
        }})
    }

    #[test]
    fn defaults_and_merge() {
        assert_eq!(defaults(&schema()), json!({ "limit": 50, "mode": "a" }));
        assert_eq!(
            merge_with_defaults(&schema(), &json!({ "limit": 10 })),
            json!({ "limit": 10, "mode": "a" })
        );
    }

    #[test]
    fn validation() {
        assert!(validate(&schema(), &json!({ "limit": 5 })).is_ok());
        assert!(validate(&schema(), &json!({ "limit": "5" })).is_err());
        assert!(validate(&schema(), &json!({ "mode": "c" })).is_err());
        assert!(validate(&schema(), &json!({ "other": 1 })).is_err());
    }
}
