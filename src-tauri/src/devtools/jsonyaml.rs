//! JSON/YAML Tools: format, minify, validate, convert JSON ⇄ YAML, JSONPath
//! queries. Errors carry line and column. Nothing is stored.

use serde::Serialize;
use serde_json::{Map, Value};
use tauri::AppHandle;

use super::{require, setting_u64};
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::DEV_JSON, registry};

const MAX_INPUT: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParseError {
    pub message: String,
    /// 1-based; `None` when the parser gives no position.
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
}

impl Format {
    fn name(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Yaml => "yaml",
        }
    }
}

pub fn parse_json(text: &str) -> Result<Value, ParseError> {
    serde_json::from_str(text).map_err(|e| ParseError { message: json_message(&e), line: Some(e.line()), column: Some(e.column()) })
}

fn json_message(e: &serde_json::Error) -> String {
    // serde_json appends " at line X column Y"; the position is shown separately.
    let s = e.to_string();
    match s.rfind(" at line ") {
        Some(i) => s[..i].to_string(),
        None => s,
    }
}

pub fn parse_yaml(text: &str) -> Result<Value, ParseError> {
    let v: serde_norway::Value = serde_norway::from_str(text).map_err(|e| {
        let loc = e.location();
        let s = e.to_string();
        let message = match s.find(" at line ") {
            Some(i) => s[..i].to_string(),
            None => s,
        };
        ParseError { message, line: loc.as_ref().map(|l| l.line()), column: loc.as_ref().map(|l| l.column()) }
    })?;
    Ok(yaml_to_json(v))
}

/// YAML → JSON value: non-string keys become strings, tags are dropped,
/// non-finite numbers become strings.
fn yaml_to_json(v: serde_norway::Value) -> Value {
    use serde_norway::Value as Y;
    match v {
        Y::Null => Value::Null,
        Y::Bool(b) => Value::Bool(b),
        Y::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(u) = n.as_u64() {
                Value::from(u)
            } else {
                let f = n.as_f64().unwrap_or(f64::NAN);
                serde_json::Number::from_f64(f).map(Value::Number).unwrap_or_else(|| Value::String(n.to_string()))
            }
        }
        Y::String(s) => Value::String(s),
        Y::Sequence(seq) => Value::Array(seq.into_iter().map(yaml_to_json).collect()),
        Y::Mapping(m) => {
            let mut out = Map::new();
            for (k, v) in m {
                let key = match yaml_to_json(k) {
                    Value::String(s) => s,
                    other => other.to_string(),
                };
                out.insert(key, yaml_to_json(v));
            }
            Value::Object(out)
        }
        Y::Tagged(t) => yaml_to_json(t.value),
    }
}

/// Guesses the format: JSON if it parses as JSON or starts like JSON.
pub fn detect(text: &str) -> Format {
    let t = text.trim_start();
    if t.starts_with('{') || t.starts_with('[') || parse_json(text).is_ok() { Format::Json } else { Format::Yaml }
}

pub fn parse(text: &str, format: Format) -> Result<Value, ParseError> {
    match format {
        Format::Json => parse_json(text),
        Format::Yaml => parse_yaml(text),
    }
}

pub fn sort_keys(v: Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut entries: Vec<(String, Value)> = m.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().map(|(k, v)| (k, sort_keys(v))).collect())
        }
        Value::Array(a) => Value::Array(a.into_iter().map(sort_keys).collect()),
        other => other,
    }
}

pub fn to_json(v: &Value, indent: usize) -> String {
    let pad = vec![b' '; indent.clamp(1, 8)];
    let mut out = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    v.serialize(&mut ser).expect("Value always serializes");
    String::from_utf8(out).expect("JSON is UTF-8")
}

pub fn to_yaml(v: &Value) -> Result<String, ParseError> {
    serde_norway::to_string(v).map_err(|e| ParseError { message: e.to_string(), line: None, column: None })
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Stats {
    pub objects: usize,
    pub arrays: usize,
    pub values: usize,
    pub depth: usize,
}

pub fn stats(v: &Value) -> Stats {
    fn walk(v: &Value, depth: usize, s: &mut Stats) {
        s.depth = s.depth.max(depth);
        match v {
            Value::Object(m) => {
                s.objects += 1;
                m.values().for_each(|c| walk(c, depth + 1, s));
            }
            Value::Array(a) => {
                s.arrays += 1;
                a.iter().for_each(|c| walk(c, depth + 1, s));
            }
            _ => s.values += 1,
        }
    }
    let mut s = Stats::default();
    walk(v, 1, &mut s);
    s
}

// --- JSONPath ---

/// One step of a JSONPath expression.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Key(String),
    Index(i64),
    Wildcard,
    /// `..` before the next step.
    Descend,
    Union(Vec<Sel>),
    Slice(Option<i64>, Option<i64>, i64),
    Filter(Filter),
}

#[derive(Debug, Clone, PartialEq)]
enum Sel {
    Key(String),
    Index(i64),
}

#[derive(Debug, Clone, PartialEq)]
struct Filter {
    /// Path relative to `@`.
    path: Vec<String>,
    op: Option<(String, Value)>,
}

fn path_err(msg: impl Into<String>) -> ParseError {
    ParseError { message: msg.into(), line: None, column: None }
}

fn parse_path(expr: &str) -> Result<Vec<Step>, ParseError> {
    let e = expr.trim();
    let rest = e.strip_prefix('$').ok_or_else(|| path_err("JSONPath начинается с $"))?;
    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;
    let mut steps = Vec::new();
    while i < chars.len() {
        match chars[i] {
            '.' if chars.get(i + 1) == Some(&'.') => {
                steps.push(Step::Descend);
                i += 2;
                if chars.get(i) == Some(&'[') {
                    continue;
                }
                let (name, n) = read_name(&chars[i..]);
                if name.is_empty() {
                    return Err(path_err("после .. нужно имя, * или [...]"));
                }
                steps.push(if name == "*" { Step::Wildcard } else { Step::Key(name) });
                i += n;
            }
            '.' => {
                i += 1;
                let (name, n) = read_name(&chars[i..]);
                if name.is_empty() {
                    return Err(path_err(format!("после точки нужно имя (позиция {})", i + 1)));
                }
                steps.push(if name == "*" { Step::Wildcard } else { Step::Key(name) });
                i += n;
            }
            '[' => {
                let close = find_close(&chars, i).ok_or_else(|| path_err("не закрыта скобка ]"))?;
                let inner: String = chars[i + 1..close].iter().collect();
                steps.push(parse_bracket(inner.trim())?);
                i = close + 1;
            }
            c if c.is_whitespace() => i += 1,
            c => return Err(path_err(format!("неожиданный символ «{c}» (позиция {})", i + 1))),
        }
    }
    Ok(steps)
}

fn read_name(chars: &[char]) -> (String, usize) {
    if chars.first() == Some(&'*') {
        return ("*".into(), 1);
    }
    let n = chars.iter().take_while(|c| c.is_alphanumeric() || **c == '_' || **c == '-' || **c == '$').count();
    (chars[..n].iter().collect(), n)
}

/// Matching `]`, skipping quoted strings and nested brackets.
fn find_close(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    for (j, &c) in chars.iter().enumerate().skip(open) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '[') => depth += 1,
            (None, ']') => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    if s.len() >= 2 && ((s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"'))) {
        Some(s[1..s.len() - 1].to_string())
    } else {
        None
    }
}

fn parse_bracket(inner: &str) -> Result<Step, ParseError> {
    if inner == "*" {
        return Ok(Step::Wildcard);
    }
    if let Some(f) = inner.strip_prefix('?') {
        let f = f.trim();
        let f = f.strip_prefix('(').and_then(|f| f.strip_suffix(')')).unwrap_or(f).trim();
        return parse_filter(f).map(Step::Filter);
    }
    if inner.contains(':') && unquote(inner).is_none() {
        let parts: Vec<&str> = inner.split(':').map(str::trim).collect();
        if parts.len() > 3 {
            return Err(path_err(format!("срез [{inner}]")));
        }
        let num = |s: &str| -> Result<Option<i64>, ParseError> { if s.is_empty() { Ok(None) } else { s.parse().map(Some).map_err(|_| path_err(format!("срез [{inner}]"))) } };
        let step = num(parts.get(2).copied().unwrap_or(""))?.unwrap_or(1);
        if step == 0 {
            return Err(path_err("шаг среза не может быть 0"));
        }
        return Ok(Step::Slice(num(parts[0])?, num(parts.get(1).copied().unwrap_or(""))?, step));
    }
    let mut sels = Vec::new();
    for part in split_top(inner, ',') {
        let p = part.trim();
        if let Some(k) = unquote(p) {
            sels.push(Sel::Key(k));
        } else if let Ok(n) = p.parse::<i64>() {
            sels.push(Sel::Index(n));
        } else {
            return Err(path_err(format!("непонятный селектор [{p}]: имена берутся в кавычки")));
        }
    }
    Ok(match sels.as_slice() {
        [Sel::Key(k)] => Step::Key(k.clone()),
        [Sel::Index(n)] => Step::Index(*n),
        _ => Step::Union(sels),
    })
}

fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '\'' | '"') => quote = Some(c),
            (None, c) if c == sep => {
                out.push(String::new());
                continue;
            }
            _ => {}
        }
        out.last_mut().unwrap().push(c);
    }
    out
}

fn parse_filter(f: &str) -> Result<Filter, ParseError> {
    let ops = ["==", "!=", "<=", ">=", "<", ">"];
    let (lhs, op) = match ops.iter().filter_map(|op| f.find(op).map(|i| (i, *op))).min_by_key(|(i, op)| (*i, std::cmp::Reverse(op.len()))) {
        Some((i, op)) => {
            let rhs = f[i + op.len()..].trim();
            let value = unquote(rhs).map(Value::String).or_else(|| serde_json::from_str(rhs).ok()).ok_or_else(|| path_err(format!("значение в фильтре: {rhs}")))?;
            (f[..i].trim(), Some((op.to_string(), value)))
        }
        None => (f, None),
    };
    let rel = lhs.strip_prefix('@').ok_or_else(|| path_err("фильтр: слева нужен @, например [?(@.price < 10)]"))?;
    let path = rel.split('.').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    Ok(Filter { path, op })
}

fn compare(a: &Value, op: &str, b: &Value) -> bool {
    match op {
        "==" => a == b,
        "!=" => a != b,
        _ => {
            let ord = match (a, b) {
                (Value::Number(x), Value::Number(y)) => x.as_f64().partial_cmp(&y.as_f64()),
                (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
                _ => None,
            };
            matches!((op, ord), ("<", Some(std::cmp::Ordering::Less)) | ("<=", Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)) | (">", Some(std::cmp::Ordering::Greater)) | (">=", Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)))
        }
    }
}

fn filter_matches(f: &Filter, v: &Value) -> bool {
    let mut cur = Some(v);
    for k in &f.path {
        cur = cur.and_then(|c| match c {
            Value::Object(m) => m.get(k),
            Value::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get(i)),
            _ => None,
        });
    }
    match (&f.op, cur) {
        (None, found) => found.is_some(),
        (Some((op, rhs)), Some(found)) => compare(found, op, rhs),
        (Some(_), None) => false,
    }
}

fn key_path(base: &str, k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_') && !k.chars().next().unwrap().is_ascii_digit() {
        format!("{base}.{k}")
    } else {
        format!("{base}['{}']", k.replace('\'', "\\'"))
    }
}

fn index(len: usize, i: i64) -> Option<usize> {
    let i = if i < 0 { len as i64 + i } else { i };
    (0..len as i64).contains(&i).then_some(i as usize)
}

fn children(v: &Value, base: &str) -> Vec<(String, Value)> {
    match v {
        Value::Object(m) => m.iter().map(|(k, c)| (key_path(base, k), c.clone())).collect(),
        Value::Array(a) => a.iter().enumerate().map(|(i, c)| (format!("{base}[{i}]"), c.clone())).collect(),
        _ => vec![],
    }
}

fn descendants(v: &Value, base: &str, out: &mut Vec<(String, Value)>) {
    out.push((base.to_string(), v.clone()));
    for (p, c) in children(v, base) {
        descendants(&c, &p, out);
    }
}

fn apply(step: &Step, nodes: Vec<(String, Value)>) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for (p, v) in nodes {
        match step {
            Step::Key(k) => {
                if let Some(c) = v.get(k) {
                    out.push((key_path(&p, k), c.clone()));
                }
            }
            Step::Index(i) => {
                if let Value::Array(a) = &v {
                    if let Some(i) = index(a.len(), *i) {
                        out.push((format!("{p}[{i}]"), a[i].clone()));
                    }
                }
            }
            Step::Wildcard => out.extend(children(&v, &p)),
            Step::Descend => descendants(&v, &p, &mut out),
            Step::Union(sels) => {
                for s in sels {
                    let sub = match s {
                        Sel::Key(k) => Step::Key(k.clone()),
                        Sel::Index(i) => Step::Index(*i),
                    };
                    out.extend(apply(&sub, vec![(p.clone(), v.clone())]));
                }
            }
            Step::Slice(start, end, step) => {
                if let Value::Array(a) = &v {
                    let len = a.len() as i64;
                    let norm = |x: i64| if x < 0 { (len + x).max(0) } else { x.min(len) };
                    let mut idx: Vec<i64> = Vec::new();
                    if *step > 0 {
                        let (mut i, e) = (norm(start.unwrap_or(0)), norm(end.unwrap_or(len)));
                        while i < e {
                            idx.push(i);
                            i += step;
                        }
                    } else {
                        let mut i = start.map(norm).unwrap_or(len - 1).min(len - 1);
                        let e = end.map(norm).unwrap_or(-1);
                        while i > e {
                            idx.push(i);
                            i += step;
                        }
                    }
                    out.extend(idx.into_iter().map(|i| (format!("{p}[{i}]"), a[i as usize].clone())));
                }
            }
            Step::Filter(f) => out.extend(children(&v, &p).into_iter().filter(|(_, c)| filter_matches(f, c))),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Match {
    pub path: String,
    pub value: Value,
}

/// Evaluates a JSONPath: `$`, `.key`, `['key']`, `[0]`, `[-1]`, `[*]`, `..key`,
/// `[a,b]`, `[start:end:step]`, `[?(@.x == 1)]`, `[?(@.x)]`.
pub fn query(v: &Value, expr: &str) -> Result<Vec<Match>, ParseError> {
    let steps = parse_path(expr)?;
    let mut nodes = vec![("$".to_string(), v.clone())];
    let mut i = 0;
    while i < steps.len() {
        // `..x`: descendants then x on each of them.
        nodes = apply(&steps[i], nodes);
        if steps[i] == Step::Descend {
            if let Some(next) = steps.get(i + 1) {
                nodes = apply(next, nodes);
                i += 1;
            }
            // Wildcard after .. would list each node twice ($ itself included).
            if steps.get(i) == Some(&Step::Wildcard) {
                nodes.retain(|(p, _)| p != "$");
                nodes.dedup_by(|a, b| a.0 == b.0);
            }
        }
        i += 1;
    }
    Ok(nodes.into_iter().map(|(path, value)| Match { path, value }).collect())
}

// --- commands ---

#[derive(Debug, Clone, Serialize)]
pub struct Processed {
    pub ok: bool,
    pub output: String,
    /// Format of the input: json or yaml.
    pub detected: &'static str,
    pub error: Option<ParseError>,
    pub stats: Option<Stats>,
}

fn input_format(text: &str, from: Option<&str>) -> Format {
    match from {
        Some("json") => Format::Json,
        Some("yaml") => Format::Yaml,
        _ => detect(text),
    }
}

/// `op`: format, minify, validate, to_json, to_yaml.
pub fn process(text: &str, from: Option<&str>, op: &str, indent: usize, sort: bool) -> AppResult<Processed> {
    if text.len() > MAX_INPUT {
        return Err(AppError::Invalid("текст больше 20 МБ".into()));
    }
    let format = input_format(text, from);
    let fail = |e: ParseError| Processed { ok: false, output: String::new(), detected: format.name(), error: Some(e), stats: None };
    if text.trim().is_empty() {
        return Ok(fail(path_err("пустой ввод")));
    }
    let value = match parse(text, format) {
        Ok(v) => v,
        Err(e) => return Ok(fail(e)),
    };
    let value = if sort { sort_keys(value) } else { value };
    let output = match (op, format) {
        ("validate", _) => String::new(),
        ("minify", _) => value.to_string(),
        ("to_yaml", _) | ("format", Format::Yaml) => match to_yaml(&value) {
            Ok(y) => y,
            Err(e) => return Ok(fail(e)),
        },
        ("to_json", _) | ("format", Format::Json) => to_json(&value, indent),
        (other, _) => return Err(AppError::Invalid(format!("неизвестная операция {other}"))),
    };
    Ok(Processed { ok: true, output, detected: format.name(), error: None, stats: Some(stats(&value)) })
}

#[tauri::command]
pub async fn devjson_process(app: AppHandle, text: String, from: Option<String>, op: String) -> AppResult<Processed> {
    require(&app, DEV_JSON)?;
    let indent = setting_u64(&app, DEV_JSON, "indent", 2).await as usize;
    let db = &tauri::Manager::state::<crate::state::AppState>(&app).db;
    let sort = registry::settings(db, DEV_JSON).await?.get("sort_keys").and_then(Value::as_bool).unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || process(&text, from.as_deref(), &op, indent, sort)).await.map_err(|e| AppError::Other(e.into()))?
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub matches: Vec<Match>,
    pub error: Option<ParseError>,
    /// The error is in the document, not in the path.
    pub in_document: bool,
}

#[tauri::command]
pub async fn devjson_query(app: AppHandle, text: String, from: Option<String>, path: String) -> AppResult<QueryResult> {
    require(&app, DEV_JSON)?;
    tauri::async_runtime::spawn_blocking(move || {
        let format = input_format(&text, from.as_deref());
        let value = match parse(&text, format) {
            Ok(v) => v,
            Err(e) => return QueryResult { matches: vec![], error: Some(e), in_document: true },
        };
        match query(&value, &path) {
            Ok(mut m) => {
                m.truncate(5000);
                QueryResult { matches: m, error: None, in_document: false }
            }
            Err(e) => QueryResult { matches: vec![], error: Some(e), in_document: false },
        }
    })
    .await
    .map_err(|e| AppError::Other(e.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const STORE: &str = r#"{"store":{"book":[
        {"title":"A","price":8.95,"isbn":"1"},
        {"title":"B","price":12.99},
        {"title":"C","price":22.5,"isbn":"3"}],
        "bicycle":{"color":"red","price":19.95},
        "odd key":1}}"#;

    fn paths(expr: &str) -> Vec<String> {
        query(&parse_json(STORE).unwrap(), expr).unwrap().into_iter().map(|m| m.path).collect()
    }

    #[test]
    fn jsonpath() {
        assert_eq!(paths("$.store.book[0].title"), ["$.store.book[0].title"]);
        assert_eq!(paths("$.store.book[-1]"), ["$.store.book[2]"]);
        assert_eq!(paths("$.store.book[*].title").len(), 3);
        assert_eq!(paths("$..price").len(), 4);
        assert_eq!(paths("$.store.book[0:2]"), ["$.store.book[0]", "$.store.book[1]"]);
        assert_eq!(paths("$.store.book[::-1]"), ["$.store.book[2]", "$.store.book[1]", "$.store.book[0]"]);
        assert_eq!(paths("$.store.book[0,2].title"), ["$.store.book[0].title", "$.store.book[2].title"]);
        assert_eq!(paths("$.store.book[?(@.price < 10)].title"), ["$.store.book[0].title"]);
        assert_eq!(paths("$.store.book[?(@.isbn)]").len(), 2);
        assert_eq!(paths("$.store.book[?(@.title == 'B')]"), ["$.store.book[1]"]);
        assert_eq!(paths("$.store['odd key']"), ["$.store['odd key']"]);
        assert_eq!(paths("$.store.*").len(), 3);
        assert_eq!(paths("$"), ["$"]);
        assert!(paths("$.missing").is_empty());
        let v = parse_json(STORE).unwrap();
        assert!(query(&v, "store").is_err());
        assert!(query(&v, "$.store[").is_err());
        assert!(query(&v, "$[?(price > 1)]").is_err());
        assert!(query(&v, "$.a[::0]").is_err());
    }

    #[test]
    fn errors_have_positions() {
        let e = parse_json("{\n  \"a\": 1,\n  \"b\": }").unwrap_err();
        assert_eq!((e.line, e.column), (Some(3), Some(8)));
        assert!(!e.message.contains("at line"));
        let e = parse_yaml("a: 1\nb: [1, 2\n").unwrap_err();
        assert!(e.line.is_some());
    }

    #[test]
    fn convert_and_format() {
        let yaml = "name: FocusDeck\nversion: 1\ntags:\n  - rust\n  - tauri\n1: numeric key\n";
        assert_eq!(detect(yaml), Format::Yaml);
        let p = process(yaml, None, "to_json", 2, false).unwrap();
        assert!(p.ok);
        let v: Value = serde_json::from_str(&p.output).unwrap();
        assert_eq!(v, json!({ "name": "FocusDeck", "version": 1, "tags": ["rust", "tauri"], "1": "numeric key" }));
        assert!(p.output.starts_with("{\n  \"name\""), "key order is kept");
        let back = process(&p.output, Some("json"), "to_yaml", 2, false).unwrap();
        assert_eq!(parse_yaml(&back.output).unwrap(), v);
        assert_eq!(process("{\"b\":1,\"a\":[1,2]}", None, "minify", 2, true).unwrap().output, "{\"a\":[1,2],\"b\":1}");
        assert_eq!(process("{\"a\":1}", None, "format", 4, false).unwrap().output, "{\n    \"a\": 1\n}");
        let bad = process("{\"a\":", None, "validate", 2, false).unwrap();
        assert!(!bad.ok && bad.error.is_some());
        assert_eq!(stats(&json!({ "a": [1, { "b": 2 }] })), Stats { objects: 2, arrays: 1, values: 2, depth: 4 });
        assert!(process("x", None, "explode", 2, false).is_err());
    }
}
