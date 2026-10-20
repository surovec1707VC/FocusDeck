//! API Tester: REST requests, collections, environments with `{{variables}}`,
//! history and export to curl.
//!
//! A request is sent only by the user and only with the `network.api_tester`
//! permission (`api.send`, logged without the query string). Only http and
//! https are allowed. The response body is shown up to the module's size limit.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use super::{guarded, require, setting_u64};
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::DEV_API, registry};
use crate::state::AppState;

pub const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
pub const BODY_TYPES: &[&str] = &["none", "json", "text", "form"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Pair {
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RequestSpec {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<Pair>,
    #[serde(default)]
    pub query: Vec<Pair>,
    #[serde(default)]
    pub body: String,
    #[serde(default = "none")]
    pub body_type: String,
}

fn none() -> String {
    "none".into()
}

impl RequestSpec {
    fn validate(&self) -> AppResult<()> {
        if !METHODS.contains(&self.method.as_str()) {
            return Err(AppError::Invalid(format!("метод {} не поддерживается", self.method)));
        }
        if !BODY_TYPES.contains(&self.body_type.as_str()) {
            return Err(AppError::Invalid(format!("тип тела {} не поддерживается", self.body_type)));
        }
        if self.url.trim().is_empty() || self.url.len() > 8192 {
            return Err(AppError::Invalid("адрес: от 1 до 8192 символов".into()));
        }
        if self.headers.len() > 100 || self.query.len() > 200 {
            return Err(AppError::Invalid("слишком много заголовков или параметров".into()));
        }
        if self.body.len() > 10 * 1024 * 1024 {
            return Err(AppError::Invalid("тело запроса больше 10 МБ".into()));
        }
        Ok(())
    }
}

// --- variables ---

/// Replaces `{{name}}` with enabled variables; returns the names left unknown.
pub fn substitute(text: &str, vars: &[Pair], missing: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(a) = rest.find("{{") {
        let Some(b) = rest[a + 2..].find("}}") else { break };
        out += &rest[..a];
        let name = rest[a + 2..a + 2 + b].trim();
        match vars.iter().rev().find(|v| v.enabled && v.name.trim() == name) {
            Some(v) => out += &v.value,
            None => {
                if !missing.iter().any(|m| m == name) {
                    missing.push(name.to_string());
                }
                out += &rest[a..a + 2 + b + 2];
            }
        }
        rest = &rest[a + 2 + b + 2..];
    }
    out + rest
}

/// The request with variables filled in and disabled rows removed.
pub fn resolve(spec: &RequestSpec, vars: &[Pair]) -> (RequestSpec, Vec<String>) {
    let mut missing = Vec::new();
    let mut sub = |s: &str| substitute(s, vars, &mut missing);
    let pairs = |list: &[Pair], sub: &mut dyn FnMut(&str) -> String| {
        list.iter()
            .filter(|p| p.enabled && !p.name.trim().is_empty())
            .map(|p| Pair { name: sub(p.name.trim()), value: sub(&p.value), enabled: true })
            .collect::<Vec<_>>()
    };
    let headers = pairs(&spec.headers, &mut sub);
    let query = pairs(&spec.query, &mut sub);
    let url = sub(spec.url.trim());
    let body = sub(&spec.body);
    (RequestSpec { method: spec.method.clone(), url, headers, query, body, body_type: spec.body_type.clone() }, missing)
}

/// URL with the query parameters appended.
pub fn full_url(spec: &RequestSpec) -> AppResult<reqwest::Url> {
    let mut url = reqwest::Url::parse(&spec.url).map_err(|e| AppError::Invalid(format!("адрес {}: {e}", spec.url)))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::Invalid("поддерживаются только адреса http:// и https://".into()));
    }
    if !spec.query.is_empty() {
        let mut q = url.query_pairs_mut();
        for p in &spec.query {
            q.append_pair(&p.name, &p.value);
        }
    }
    Ok(url)
}

fn content_type(spec: &RequestSpec) -> Option<&'static str> {
    match spec.body_type.as_str() {
        "json" => Some("application/json"),
        "text" => Some("text/plain; charset=utf-8"),
        "form" => Some("application/x-www-form-urlencoded"),
        _ => None,
    }
}

fn has_header(spec: &RequestSpec, name: &str) -> bool {
    spec.headers.iter().any(|h| h.name.eq_ignore_ascii_case(name))
}

// --- curl ---

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn cmd_quote(s: &str) -> String {
    // curl.exe in cmd.exe: double quotes, inner quotes escaped; % doubled is
    // only needed in batch files, so it is left as is.
    format!("\"{}\"", s.replace('"', "\\\""))
}

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// `curl` command for bash, cmd.exe or PowerShell (`curl.exe`).
pub fn to_curl(spec: &RequestSpec, style: &str) -> AppResult<String> {
    let url = full_url(spec)?;
    let (exe, quote, cont): (&str, fn(&str) -> String, &str) = match style {
        "cmd" => ("curl.exe", cmd_quote, " ^\n  "),
        "powershell" => ("curl.exe", ps_quote, " `\n  "),
        _ => ("curl", sh_quote, " \\\n  "),
    };
    let mut parts = vec![exe.to_string()];
    if spec.method == "HEAD" {
        parts.push("--head".into());
    } else if spec.method != "GET" || (spec.body_type != "none" && !spec.body.is_empty()) {
        parts.push(format!("-X {}", spec.method));
    }
    parts.push(quote(url.as_str()));
    for h in &spec.headers {
        parts.push(format!("-H {}", quote(&format!("{}: {}", h.name, h.value))));
    }
    if spec.body_type != "none" && !spec.body.is_empty() {
        if let Some(ct) = content_type(spec) {
            if !has_header(spec, "content-type") {
                parts.push(format!("-H {}", quote(&format!("Content-Type: {ct}"))));
            }
        }
        parts.push(format!("--data-raw {}", quote(&spec.body)));
    }
    Ok(parts.join(cont))
}

// --- sending ---

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// The body isn't text: `body` holds a short description.
    pub binary: bool,
    pub size: u64,
    pub truncated: bool,
    pub duration_ms: i64,
    pub final_url: String,
    /// Variables without a value in the active environment.
    pub missing_vars: Vec<String>,
}

async fn send(app: &AppHandle, spec: &RequestSpec) -> AppResult<Response> {
    let db = &app.state::<AppState>().db;
    let settings = registry::settings(db, DEV_API).await?;
    let timeout = settings.get("timeout_secs").and_then(Value::as_u64).unwrap_or(30);
    let follow = settings.get("follow_redirects").and_then(Value::as_bool).unwrap_or(true);
    let max = settings.get("max_response_kb").and_then(Value::as_u64).unwrap_or(2048) as usize * 1024;

    let url = full_url(spec)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .redirect(if follow { reqwest::redirect::Policy::limited(10) } else { reqwest::redirect::Policy::none() })
        .user_agent(concat!("FocusDeck-API-Tester/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::Other(e.into()))?;
    let method = reqwest::Method::from_bytes(spec.method.as_bytes()).map_err(|e| AppError::Invalid(e.to_string()))?;
    let mut req = client.request(method, url);
    for h in &spec.headers {
        req = req.header(h.name.as_str(), h.value.as_str());
    }
    if spec.body_type != "none" {
        if let Some(ct) = content_type(spec) {
            if !has_header(spec, "content-type") {
                req = req.header("Content-Type", ct);
            }
        }
        req = req.body(spec.body.clone());
    }
    let started = Instant::now();
    let mut resp = req.send().await.map_err(|e| AppError::Other(anyhow::anyhow!(describe(&e))))?;
    let status = resp.status();
    let headers = resp.headers().iter().map(|(k, v)| (k.to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned())).collect();
    let final_url = resp.url().to_string();
    let mut body = Vec::new();
    let mut size = 0u64;
    while let Some(chunk) = resp.chunk().await.map_err(|e| AppError::Other(anyhow::anyhow!(describe(&e))))? {
        size += chunk.len() as u64;
        if body.len() < max {
            body.extend_from_slice(&chunk[..chunk.len().min(max - body.len())]);
        }
    }
    let duration_ms = started.elapsed().as_millis() as i64;
    let truncated = size as usize > body.len();
    let (text, binary) = match String::from_utf8(body) {
        Ok(s) => (s, false),
        Err(e) if truncated && e.utf8_error().error_len().is_none() => {
            let valid = e.utf8_error().valid_up_to();
            (String::from_utf8_lossy(&e.into_bytes()[..valid]).into_owned(), false)
        }
        Err(_) => (format!("Двоичные данные, {size} байт"), true),
    };
    Ok(Response {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").into(),
        headers,
        body: text,
        binary,
        size,
        truncated,
        duration_ms,
        final_url,
        missing_vars: vec![],
    })
}

fn describe(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "превышен таймаут запроса".into()
    } else if e.is_connect() {
        format!("не удалось подключиться: {e}")
    } else if e.is_redirect() {
        "слишком много перенаправлений".into()
    } else {
        e.to_string()
    }
}

async fn add_history(pool: &SqlitePool, spec: &RequestSpec, r: &AppResult<Response>, limit: i64) -> AppResult<()> {
    let (status, duration, size, error) = match r {
        Ok(r) => (Some(r.status as i64), r.duration_ms, r.size as i64, None),
        Err(e) => (None, 0, 0, Some(e.to_string())),
    };
    sqlx::query("INSERT INTO api_history (method, url, status, duration_ms, size, error, request_json) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&spec.method)
        .bind(&spec.url)
        .bind(status)
        .bind(duration)
        .bind(size)
        .bind(error)
        .bind(serde_json::to_string(spec)?)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM api_history WHERE id <= (SELECT MAX(id) FROM api_history) - ?").bind(limit).execute(pool).await?;
    Ok(())
}

// --- collections, requests, environments, history ---

#[derive(Debug, Clone, Serialize)]
pub struct SavedRequest {
    pub id: String,
    pub collection_id: Option<String>,
    pub name: String,
    #[serde(flatten)]
    pub spec: RequestSpec,
    pub position: i64,
    pub updated_at: String,
}

#[derive(sqlx::FromRow)]
struct RequestRow {
    id: String,
    collection_id: Option<String>,
    name: String,
    method: String,
    url: String,
    headers_json: String,
    query_json: String,
    body: String,
    body_type: String,
    position: i64,
    updated_at: String,
}

impl From<RequestRow> for SavedRequest {
    fn from(r: RequestRow) -> Self {
        SavedRequest {
            id: r.id,
            collection_id: r.collection_id,
            name: r.name,
            spec: RequestSpec {
                method: r.method,
                url: r.url,
                headers: serde_json::from_str(&r.headers_json).unwrap_or_default(),
                query: serde_json::from_str(&r.query_json).unwrap_or_default(),
                body: r.body,
                body_type: r.body_type,
            },
            position: r.position,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub requests: Vec<SavedRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Workspace {
    pub collections: Vec<Collection>,
    /// Requests outside any collection.
    pub loose: Vec<SavedRequest>,
}

pub async fn workspace(pool: &SqlitePool) -> AppResult<Workspace> {
    let cols: Vec<(String, String)> = sqlx::query_as("SELECT id, name FROM api_collections ORDER BY position, created_at").fetch_all(pool).await?;
    let reqs: Vec<SavedRequest> = sqlx::query_as::<_, RequestRow>("SELECT id, collection_id, name, method, url, headers_json, query_json, body, body_type, position, updated_at FROM api_requests ORDER BY position, created_at")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(SavedRequest::from)
        .collect();
    let collections = cols
        .into_iter()
        .map(|(id, name)| Collection { requests: reqs.iter().filter(|r| r.collection_id.as_deref() == Some(&id)).cloned().collect(), id, name })
        .collect();
    Ok(Workspace { collections, loose: reqs.into_iter().filter(|r| r.collection_id.is_none()).collect() })
}

fn check_name(name: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 100 {
        return Err(AppError::Invalid("название: от 1 до 100 символов".into()));
    }
    Ok(n.into())
}

pub async fn save_collection(pool: &SqlitePool, id: Option<&str>, name: &str) -> AppResult<String> {
    let name = check_name(name)?;
    match id {
        Some(id) => {
            if sqlx::query("UPDATE api_collections SET name = ? WHERE id = ?").bind(&name).bind(id).execute(pool).await?.rows_affected() == 0 {
                return Err(AppError::NotFound("коллекция".into()));
            }
            Ok(id.into())
        }
        None => {
            let id = new_id();
            sqlx::query("INSERT INTO api_collections (id, name, position) VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM api_collections))").bind(&id).bind(&name).execute(pool).await?;
            Ok(id)
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestInput {
    pub name: String,
    #[serde(default)]
    pub collection_id: Option<String>,
    #[serde(flatten)]
    pub spec: RequestSpec,
}

pub async fn save_request(pool: &SqlitePool, id: Option<&str>, i: RequestInput) -> AppResult<SavedRequest> {
    let name = check_name(&i.name)?;
    i.spec.validate()?;
    let headers = serde_json::to_string(&i.spec.headers)?;
    let query = serde_json::to_string(&i.spec.query)?;
    let id = match id {
        Some(id) => {
            let n = sqlx::query("UPDATE api_requests SET collection_id = ?, name = ?, method = ?, url = ?, headers_json = ?, query_json = ?, body = ?, body_type = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(&i.collection_id)
                .bind(&name)
                .bind(&i.spec.method)
                .bind(i.spec.url.trim())
                .bind(&headers)
                .bind(&query)
                .bind(&i.spec.body)
                .bind(&i.spec.body_type)
                .bind(id)
                .execute(pool)
                .await?
                .rows_affected();
            if n == 0 {
                return Err(AppError::NotFound("запрос".into()));
            }
            id.to_string()
        }
        None => {
            let id = new_id();
            sqlx::query("INSERT INTO api_requests (id, collection_id, name, method, url, headers_json, query_json, body, body_type, position) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM api_requests))")
                .bind(&id)
                .bind(&i.collection_id)
                .bind(&name)
                .bind(&i.spec.method)
                .bind(i.spec.url.trim())
                .bind(&headers)
                .bind(&query)
                .bind(&i.spec.body)
                .bind(&i.spec.body_type)
                .execute(pool)
                .await?;
            id
        }
    };
    Ok(sqlx::query_as::<_, RequestRow>("SELECT id, collection_id, name, method, url, headers_json, query_json, body, body_type, position, updated_at FROM api_requests WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?
        .into())
}

#[derive(Debug, Clone, Serialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub vars: Vec<Pair>,
    pub active: bool,
}

pub async fn environments(pool: &SqlitePool) -> AppResult<Vec<Environment>> {
    let rows: Vec<(String, String, String, bool)> = sqlx::query_as("SELECT id, name, vars_json, active FROM api_environments ORDER BY created_at").fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(id, name, vars, active)| Environment { id, name, vars: serde_json::from_str(&vars).unwrap_or_default(), active }).collect())
}

pub async fn active_vars(pool: &SqlitePool) -> AppResult<Vec<Pair>> {
    Ok(environments(pool).await?.into_iter().find(|e| e.active).map(|e| e.vars).unwrap_or_default())
}

pub async fn save_environment(pool: &SqlitePool, id: Option<&str>, name: &str, vars: Vec<Pair>) -> AppResult<Environment> {
    let name = check_name(name)?;
    if vars.len() > 200 {
        return Err(AppError::Invalid("не больше 200 переменных".into()));
    }
    let vars_json = serde_json::to_string(&vars)?;
    let id = match id {
        Some(id) => {
            if sqlx::query("UPDATE api_environments SET name = ?, vars_json = ? WHERE id = ?").bind(&name).bind(&vars_json).bind(id).execute(pool).await?.rows_affected() == 0 {
                return Err(AppError::NotFound("окружение".into()));
            }
            id.to_string()
        }
        None => {
            let id = new_id();
            // The first environment becomes active.
            sqlx::query("INSERT INTO api_environments (id, name, vars_json, active) VALUES (?, ?, ?, NOT EXISTS (SELECT 1 FROM api_environments))").bind(&id).bind(&name).bind(&vars_json).execute(pool).await?;
            id
        }
    };
    environments(pool).await?.into_iter().find(|e| e.id == id).ok_or_else(|| AppError::NotFound("окружение".into()))
}

pub async fn activate_environment(pool: &SqlitePool, id: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE api_environments SET active = COALESCE(id = ?1, 0)").bind(id).execute(pool).await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub method: String,
    pub url: String,
    pub status: Option<i64>,
    pub duration_ms: i64,
    pub size: i64,
    pub error: Option<String>,
    pub request: RequestSpec,
    pub created_at: String,
}

pub async fn history(pool: &SqlitePool, limit: i64) -> AppResult<Vec<HistoryEntry>> {
    let rows: Vec<(i64, String, String, Option<i64>, i64, i64, Option<String>, String, String)> =
        sqlx::query_as("SELECT id, method, url, status, duration_ms, size, error, request_json, created_at FROM api_history ORDER BY id DESC LIMIT ?").bind(limit.clamp(1, 1000)).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(id, method, url, status, duration_ms, size, error, req, created_at)| HistoryEntry {
            id,
            method,
            url,
            status,
            duration_ms,
            size,
            error,
            request: serde_json::from_str(&req).unwrap_or_default(),
            created_at,
        })
        .collect())
}

// --- commands ---

fn pool(app: &AppHandle) -> &SqlitePool {
    &app.state::<AppState>().inner().db
}

/// Sends a request. `{{variables}}` come from the active environment.
#[tauri::command]
pub async fn api_send(app: AppHandle, request: RequestSpec, confirm: Option<bool>) -> AppResult<Response> {
    require(&app, DEV_API)?;
    request.validate()?;
    let db = pool(&app);
    let (spec, missing) = resolve(&request, &active_vars(db).await?);
    // The log gets the address without the query string (it may hold tokens).
    let target = format!("{} {}", spec.method, spec.url.split(['?', '#']).next().unwrap_or(""));
    let limit = setting_u64(&app, DEV_API, "history_limit", 500).await as i64;
    let result = guarded(&app, DEV_API, "api.send", &target, confirm.unwrap_or(false), send(&app, &spec), |r| format!("{} {}, {} мс", r.status, r.status_text, r.duration_ms)).await;
    // Requests stopped before sending (no permission, Safe Mode) aren't history.
    if !matches!(result, Err(AppError::PermissionDenied(_) | AppError::Blocked(_) | AppError::ConfirmationRequired(_) | AppError::ModuleDisabled(_))) {
        if let Err(e) = add_history(db, &spec, &result, limit).await {
            tracing::warn!("api history: {e}");
        }
    }
    result.map(|mut r| {
        r.missing_vars = missing;
        r
    })
}

/// curl command for the request with variables filled in.
#[tauri::command]
pub async fn api_to_curl(app: AppHandle, request: RequestSpec, style: Option<String>) -> AppResult<String> {
    require(&app, DEV_API)?;
    request.validate()?;
    let (spec, _) = resolve(&request, &active_vars(pool(&app)).await?);
    to_curl(&spec, style.as_deref().unwrap_or("bash"))
}

#[tauri::command]
pub async fn api_workspace(app: AppHandle) -> AppResult<Workspace> {
    require(&app, DEV_API)?;
    workspace(pool(&app)).await
}

#[tauri::command]
pub async fn api_collection_save(app: AppHandle, id: Option<String>, name: String) -> AppResult<String> {
    require(&app, DEV_API)?;
    save_collection(pool(&app), id.as_deref(), &name).await
}

/// Deletes a collection with its requests.
#[tauri::command]
pub async fn api_collection_delete(app: AppHandle, id: String) -> AppResult<()> {
    require(&app, DEV_API)?;
    sqlx::query("DELETE FROM api_collections WHERE id = ?").bind(id).execute(pool(&app)).await?;
    Ok(())
}

#[tauri::command]
pub async fn api_request_save(app: AppHandle, id: Option<String>, input: RequestInput) -> AppResult<SavedRequest> {
    require(&app, DEV_API)?;
    save_request(pool(&app), id.as_deref(), input).await
}

#[tauri::command]
pub async fn api_request_delete(app: AppHandle, id: String) -> AppResult<()> {
    require(&app, DEV_API)?;
    sqlx::query("DELETE FROM api_requests WHERE id = ?").bind(id).execute(pool(&app)).await?;
    Ok(())
}

#[tauri::command]
pub async fn api_environments(app: AppHandle) -> AppResult<Vec<Environment>> {
    require(&app, DEV_API)?;
    environments(pool(&app)).await
}

#[tauri::command]
pub async fn api_environment_save(app: AppHandle, id: Option<String>, name: String, vars: Vec<Pair>) -> AppResult<Environment> {
    require(&app, DEV_API)?;
    save_environment(pool(&app), id.as_deref(), &name, vars).await
}

#[tauri::command]
pub async fn api_environment_delete(app: AppHandle, id: String) -> AppResult<()> {
    require(&app, DEV_API)?;
    sqlx::query("DELETE FROM api_environments WHERE id = ?").bind(id).execute(pool(&app)).await?;
    Ok(())
}

/// Makes an environment active (`null` = no environment).
#[tauri::command]
pub async fn api_environment_activate(app: AppHandle, id: Option<String>) -> AppResult<()> {
    require(&app, DEV_API)?;
    activate_environment(pool(&app), id.as_deref()).await
}

#[tauri::command]
pub async fn api_history(app: AppHandle, limit: Option<i64>) -> AppResult<Vec<HistoryEntry>> {
    require(&app, DEV_API)?;
    history(pool(&app), limit.unwrap_or(100)).await
}

#[tauri::command]
pub async fn api_history_clear(app: AppHandle) -> AppResult<()> {
    require(&app, DEV_API)?;
    sqlx::query("DELETE FROM api_history").execute(pool(&app)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(n: &str, v: &str) -> Pair {
        Pair { name: n.into(), value: v.into(), enabled: true }
    }

    fn spec(method: &str, url: &str) -> RequestSpec {
        RequestSpec { method: method.into(), url: url.into(), body_type: "none".into(), ..Default::default() }
    }

    #[test]
    fn variables_are_substituted() {
        let vars = vec![pair("host", "api.local"), Pair { name: "token".into(), value: "x".into(), enabled: false }];
        let mut missing = vec![];
        assert_eq!(substitute("https://{{host}}/v1?t={{ token }}", &vars, &mut missing), "https://api.local/v1?t={{ token }}");
        assert_eq!(missing, ["token"]);
        let mut s = spec("POST", "https://{{host}}/items");
        s.headers = vec![pair("X-Env", "{{host}}"), Pair { name: "Off".into(), value: "1".into(), enabled: false }, pair(" ", "skip")];
        s.body = r#"{"h":"{{host}}"}"#.into();
        let (r, missing) = resolve(&s, &vars);
        assert!(missing.is_empty());
        assert_eq!(r.url, "https://api.local/items");
        assert_eq!(r.headers, vec![pair("X-Env", "api.local")]);
        assert_eq!(r.body, r#"{"h":"api.local"}"#);
    }

    #[test]
    fn urls_and_validation() {
        let mut s = spec("GET", "https://example.com/a?x=1");
        s.query = vec![pair("q", "a b&c")];
        assert_eq!(full_url(&s).unwrap().as_str(), "https://example.com/a?x=1&q=a+b%26c");
        assert!(full_url(&spec("GET", "file:///C:/Windows")).is_err());
        assert!(full_url(&spec("GET", "not a url")).is_err());
        assert!(spec("TRACE", "https://x").validate().is_err());
        assert!(spec("GET", " ").validate().is_err());
    }

    #[test]
    fn curl_export() {
        let mut s = spec("POST", "https://api.local/items");
        s.headers = vec![pair("Authorization", "Bearer it's")];
        s.body_type = "json".into();
        s.body = r#"{"a":"b"}"#.into();
        assert_eq!(
            to_curl(&s, "bash").unwrap(),
            "curl \\\n  -X POST \\\n  'https://api.local/items' \\\n  -H 'Authorization: Bearer it'\\''s' \\\n  -H 'Content-Type: application/json' \\\n  --data-raw '{\"a\":\"b\"}'"
        );
        let cmd = to_curl(&s, "cmd").unwrap();
        assert!(cmd.starts_with("curl.exe ^\n  -X POST ^\n  \"https://api.local/items\""));
        assert!(cmd.ends_with("--data-raw \"{\\\"a\\\":\\\"b\\\"}\""));
        assert!(to_curl(&s, "powershell").unwrap().contains("-H 'Authorization: Bearer it''s'"));
        assert_eq!(to_curl(&spec("GET", "https://x.local/"), "bash").unwrap(), "curl \\\n  'https://x.local/'");
        assert!(to_curl(&spec("HEAD", "https://x.local/"), "bash").unwrap().contains("--head"));
    }

    #[tokio::test]
    async fn storage() {
        let pool = crate::db::test_pool().await;
        let col = save_collection(&pool, None, "Сервис").await.unwrap();
        let input = |name: &str, col: Option<String>| RequestInput { name: name.into(), collection_id: col, spec: spec("GET", "https://{{host}}/") };
        let a = save_request(&pool, None, input("Список", Some(col.clone()))).await.unwrap();
        save_request(&pool, None, input("Без коллекции", None)).await.unwrap();
        let ws = workspace(&pool).await.unwrap();
        assert_eq!(ws.collections[0].requests[0].id, a.id);
        assert_eq!(ws.loose.len(), 1);
        let mut bad = input("x", None);
        bad.spec.method = "BREW".into();
        assert!(save_request(&pool, None, bad).await.is_err());
        sqlx::query("DELETE FROM api_collections WHERE id = ?").bind(&col).execute(&pool).await.unwrap();
        assert!(workspace(&pool).await.unwrap().collections.is_empty(), "requests go with the collection");

        let dev = save_environment(&pool, None, "dev", vec![pair("host", "dev.local")]).await.unwrap();
        assert!(dev.active, "first environment is active");
        let prod = save_environment(&pool, None, "prod", vec![pair("host", "prod.local")]).await.unwrap();
        assert!(!prod.active);
        activate_environment(&pool, Some(&prod.id)).await.unwrap();
        assert_eq!(active_vars(&pool).await.unwrap()[0].value, "prod.local");
        activate_environment(&pool, None).await.unwrap();
        assert!(active_vars(&pool).await.unwrap().is_empty());

        let s = spec("GET", "https://x.local/");
        add_history(&pool, &s, &Err(AppError::Other(anyhow::anyhow!("нет сети"))), 2).await.unwrap();
        for _ in 0..3 {
            add_history(&pool, &s, &Err(AppError::Other(anyhow::anyhow!("x"))), 2).await.unwrap();
        }
        let h = history(&pool, 10).await.unwrap();
        assert_eq!(h.len(), 2, "history is bounded");
        assert_eq!(h[0].request, s);
    }

    /// A real request to a local one-shot HTTP server.
    #[tokio::test]
    async fn sends_to_local_server() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            sock.write_all(b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}").await.unwrap();
            req
        });
        let client = reqwest::Client::new();
        let mut s = spec("POST", &format!("http://{addr}/items"));
        s.query = vec![pair("q", "1")];
        let resp = client.post(full_url(&s).unwrap()).header("X-Test", "1").body("hi").send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 201);
        assert_eq!(resp.text().await.unwrap(), "{\"ok\":true}");
        let req = server.await.unwrap();
        assert!(req.starts_with("POST /items?q=1 HTTP/1.1"));
        assert!(req.to_lowercase().contains("x-test: 1"));
    }
}
