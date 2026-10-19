//! Settings helpers driven by a module's JSON schema (subset: `type`,
//! `default`, `minimum`, `maximum`, `enum`). Unknown keys are rejected.

use serde_json::{Map, Value};

fn properties(schema: &Value) -> Option<&Map<String, Value>> {
    schema.get("properties").and_then(Value::as_object)
}

pub fn defaults(schema: &Value) -> Value {
    let mut out = Map::new();
    for (k, p) in properties(schema).into_iter().flatten() {
        if let Some(d) = p.get("default") {
            out.insert(k.clone(), d.clone());
        }
    }
    Value::Object(out)
}

/// Stored values over defaults; keys no longer in the schema are dropped.
pub fn merge(schema: &Value, stored: &Value) -> Value {
    let mut merged = defaults(schema);
    if let (Some(target), Some(src), Some(props)) = (merged.as_object_mut(), stored.as_object(), properties(schema)) {
        for (k, v) in src {
            if props.contains_key(k) {
                target.insert(k.clone(), v.clone());
            }
        }
    }
    merged
}

pub fn validate(schema: &Value, settings: &Value) -> Result<(), String> {
    let obj = settings.as_object().ok_or("настройки должны быть объектом")?;
    let empty = Map::new();
    let props = properties(schema).unwrap_or(&empty);
    for (key, value) in obj {
        let p = props.get(key).ok_or_else(|| format!("неизвестная настройка «{key}»"))?;
        let title = p.get("title").and_then(Value::as_str).unwrap_or(key);
        let ok = match p.get("type").and_then(Value::as_str) {
            Some("boolean") => value.is_boolean(),
            Some("integer") => value.is_i64() || value.is_u64(),
            Some("number") => value.is_number(),
            Some("string") => value.is_string(),
            _ => true,
        };
        if !ok {
            return Err(format!("«{title}»: неверный тип значения"));
        }
        if let Some(n) = value.as_f64() {
            if let Some(min) = p.get("minimum").and_then(Value::as_f64) {
                if n < min {
                    return Err(format!("«{title}»: не меньше {min}"));
                }
            }
            if let Some(max) = p.get("maximum").and_then(Value::as_f64) {
                if n > max {
                    return Err(format!("«{title}»: не больше {max}"));
                }
            }
        }
        if let Some(allowed) = p.get("enum").and_then(Value::as_array) {
            if !allowed.contains(value) {
                return Err(format!("«{title}»: недопустимое значение"));
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
        json!({ "type": "object", "properties": {
            "n": { "type": "integer", "default": 30, "minimum": 10, "maximum": 600 },
            "level": { "type": "string", "default": "info", "enum": ["info", "debug"] },
            "flag": { "type": "boolean", "default": true }
        }})
    }

    #[test]
    fn defaults_merge_validate() {
        assert_eq!(defaults(&schema()), json!({ "n": 30, "level": "info", "flag": true }));
        assert_eq!(
            merge(&schema(), &json!({ "n": 60, "removed": 1 })),
            json!({ "n": 60, "level": "info", "flag": true })
        );
        assert!(validate(&schema(), &json!({ "n": 60, "level": "debug" })).is_ok());
        assert!(validate(&schema(), &json!({ "n": 5 })).is_err());
        assert!(validate(&schema(), &json!({ "n": 601 })).is_err());
        assert!(validate(&schema(), &json!({ "n": "60" })).is_err());
        assert!(validate(&schema(), &json!({ "level": "trace" })).is_err());
        assert!(validate(&schema(), &json!({ "x": 1 })).is_err());
        assert!(validate(&schema(), &json!([])).is_err());
    }
}
