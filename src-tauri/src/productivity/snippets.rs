//! Snippets: text templates with tags and variables, copy/paste, JSON
//! import/export.
//!
//! Variables: `{{имя}}` — asked when the snippet is used; built-in
//! `{{date}}`, `{{time}}`, `{{datetime}}`, `{{weekday}}`.

use std::collections::HashMap;

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use super::{normalize_tags, parse_tags, pick_open_text, pick_save_path, require};
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::SNIPPETS, registry};
use crate::permissions::{self, PERM_PASTE};
use crate::state::AppState;

pub const FORMAT: &str = "focusdeck.snippets";
pub const BUILTINS: &[&str] = &["date", "time", "datetime", "weekday"];

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    title: String,
    body: String,
    shortcut: Option<String>,
    tags_json: String,
    use_count: i64,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snippet {
    pub id: String,
    pub title: String,
    pub body: String,
    /// Short keyword for search, e.g. "sig".
    pub shortcut: Option<String>,
    pub tags: Vec<String>,
    /// Variables the user fills in (built-ins excluded).
    pub variables: Vec<String>,
    pub use_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct SnippetInput {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Names inside `{{…}}` in order of appearance, without duplicates.
pub fn variables(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(a) = rest.find("{{") {
        let Some(b) = rest[a + 2..].find("}}") else { break };
        let name = rest[a + 2..a + 2 + b].trim().to_string();
        if !name.is_empty() && name.chars().count() <= 40 && !name.contains('{') && !out.contains(&name) {
            out.push(name);
        }
        rest = &rest[a + 2 + b + 2..];
    }
    out
}

/// Fills variables; built-ins use the local clock, unknown names stay empty.
pub fn render(body: &str, values: &HashMap<String, String>) -> String {
    let now = Local::now();
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(a) = rest.find("{{") {
        out += &rest[..a];
        let Some(b) = rest[a + 2..].find("}}") else {
            out += &rest[a..];
            return out;
        };
        let name = rest[a + 2..a + 2 + b].trim();
        let value = match (values.get(name), name) {
            (Some(v), _) => v.clone(),
            (None, "date") => now.format("%d.%m.%Y").to_string(),
            (None, "time") => now.format("%H:%M").to_string(),
            (None, "datetime") => now.format("%d.%m.%Y %H:%M").to_string(),
            (None, "weekday") => ["понедельник", "вторник", "среда", "четверг", "пятница", "суббота", "воскресенье"]
                [chrono::Datelike::weekday(&now).num_days_from_monday() as usize]
                .to_string(),
            _ => String::new(),
        };
        out += &value;
        rest = &rest[a + 2 + b + 2..];
    }
    out + rest
}

fn to_snippet(r: Row) -> Snippet {
    Snippet {
        variables: variables(&r.body).into_iter().filter(|v| !BUILTINS.contains(&v.as_str())).collect(),
        tags: parse_tags(&r.tags_json),
        id: r.id,
        title: r.title,
        body: r.body,
        shortcut: r.shortcut,
        use_count: r.use_count,
        updated_at: r.updated_at,
    }
}

fn check(input: &SnippetInput) -> AppResult<()> {
    let n = input.title.trim().chars().count();
    if n == 0 || n > 100 {
        return Err(AppError::Invalid("название сниппета — от 1 до 100 символов".into()));
    }
    if input.body.is_empty() || input.body.chars().count() > 50_000 {
        return Err(AppError::Invalid("текст сниппета — от 1 до 50 000 символов".into()));
    }
    if input.shortcut.as_ref().is_some_and(|s| s.chars().count() > 30 || s.contains(char::is_whitespace)) {
        return Err(AppError::Invalid("сокращение — до 30 символов, без пробелов".into()));
    }
    Ok(())
}

pub async fn list(pool: &SqlitePool, search: Option<&str>, tag: Option<&str>) -> AppResult<Vec<Snippet>> {
    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, title, body, shortcut, tags_json, use_count, updated_at FROM snippets ORDER BY use_count DESC, title",
    )
    .fetch_all(pool)
    .await?;
    let search = search.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    Ok(rows
        .into_iter()
        .map(to_snippet)
        .filter(|s| {
            search.as_ref().is_none_or(|q| {
                s.title.to_lowercase().contains(q)
                    || s.body.to_lowercase().contains(q)
                    || s.shortcut.as_ref().is_some_and(|k| k.to_lowercase().contains(q))
                    || s.tags.iter().any(|t| t.contains(q))
            })
        })
        .filter(|s| tag.is_none_or(|t| s.tags.iter().any(|x| x == t)))
        .collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Snippet> {
    sqlx::query_as::<_, Row>("SELECT id, title, body, shortcut, tags_json, use_count, updated_at FROM snippets WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(to_snippet)
        .ok_or_else(|| AppError::NotFound("сниппет".into()))
}

pub async fn save(pool: &SqlitePool, id: Option<&str>, input: SnippetInput) -> AppResult<Snippet> {
    check(&input)?;
    let tags = serde_json::to_string(&normalize_tags(&input.tags)?)?;
    let shortcut = input.shortcut.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let id = match id {
        Some(id) => {
            let n = sqlx::query("UPDATE snippets SET title = ?, body = ?, shortcut = ?, tags_json = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(input.title.trim())
                .bind(&input.body)
                .bind(shortcut)
                .bind(&tags)
                .bind(id)
                .execute(pool)
                .await?
                .rows_affected();
            if n == 0 {
                return Err(AppError::NotFound("сниппет".into()));
            }
            id.to_string()
        }
        None => {
            let id = new_id();
            sqlx::query("INSERT INTO snippets (id, title, body, shortcut, tags_json) VALUES (?, ?, ?, ?, ?)")
                .bind(&id)
                .bind(input.title.trim())
                .bind(&input.body)
                .bind(shortcut)
                .bind(&tags)
                .execute(pool)
                .await?;
            id
        }
    };
    get(pool, &id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM snippets WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

/// Rendered text; counts the use.
pub async fn use_snippet(pool: &SqlitePool, id: &str, values: &HashMap<String, String>) -> AppResult<String> {
    let s = get(pool, id).await?;
    sqlx::query("UPDATE snippets SET use_count = use_count + 1 WHERE id = ?").bind(id).execute(pool).await?;
    Ok(render(&s.body, values))
}

pub async fn export_json(pool: &SqlitePool) -> AppResult<String> {
    let items: Vec<Value> = list(pool, None, None)
        .await?
        .into_iter()
        .map(|s| serde_json::json!({ "title": s.title, "body": s.body, "shortcut": s.shortcut, "tags": s.tags }))
        .collect();
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "format": FORMAT,
        "version": 1,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "snippets": items,
    }))?)
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub imported: usize,
    /// Same title and text already exist.
    pub duplicates: usize,
    pub skipped: Vec<String>,
}

pub async fn import_json(pool: &SqlitePool, json: &str) -> AppResult<ImportReport> {
    let doc: Value = serde_json::from_str(json).map_err(|e| AppError::Invalid(format!("это не JSON: {e}")))?;
    if doc["format"] != FORMAT {
        return Err(AppError::Invalid(format!("ожидался файл формата «{FORMAT}»")));
    }
    if doc["version"].as_i64().unwrap_or(0) > 1 {
        return Err(AppError::Invalid("файл создан более новой версией FocusDeck".into()));
    }
    let items = doc["snippets"].as_array().ok_or_else(|| AppError::Invalid("в файле нет списка snippets".into()))?;
    let existing = list(pool, None, None).await?;
    let mut report = ImportReport::default();
    for (i, item) in items.iter().enumerate() {
        let input = SnippetInput {
            title: item["title"].as_str().unwrap_or("").to_string(),
            body: item["body"].as_str().unwrap_or("").to_string(),
            shortcut: item["shortcut"].as_str().map(String::from),
            tags: item["tags"].as_array().map(|t| t.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        };
        if existing.iter().any(|e| e.title == input.title.trim() && e.body == input.body) {
            report.duplicates += 1;
            continue;
        }
        match save(pool, None, input).await {
            Ok(_) => report.imported += 1,
            Err(e) => report.skipped.push(format!("№{}: {e}", i + 1)),
        }
    }
    Ok(report)
}

// --- commands ---

#[tauri::command]
pub async fn snippets_list(state: State<'_, AppState>, search: Option<String>, tag: Option<String>) -> AppResult<Vec<Snippet>> {
    require(&state, SNIPPETS)?;
    list(&state.db, search.as_deref(), tag.as_deref()).await
}

#[tauri::command]
pub async fn snippets_save(state: State<'_, AppState>, id: Option<String>, input: SnippetInput) -> AppResult<Snippet> {
    require(&state, SNIPPETS)?;
    save(&state.db, id.as_deref(), input).await
}

#[tauri::command]
pub async fn snippets_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    require(&state, SNIPPETS)?;
    delete(&state.db, &id).await
}

#[tauri::command]
pub async fn snippets_render(state: State<'_, AppState>, id: String, values: Option<HashMap<String, String>>) -> AppResult<String> {
    require(&state, SNIPPETS)?;
    Ok(render(&get(&state.db, &id).await?.body, &values.unwrap_or_default()))
}

/// Puts the rendered snippet on the clipboard.
#[tauri::command]
pub async fn snippets_copy(state: State<'_, AppState>, id: String, values: Option<HashMap<String, String>>) -> AppResult<String> {
    require(&state, SNIPPETS)?;
    let text = use_snippet(&state.db, &id, &values.unwrap_or_default()).await?;
    crate::widgets::clipboard::copy(text.clone()).await?;
    Ok(text)
}

/// Copies, hides FocusDeck so the previous program gets focus, and presses Ctrl+V there.
#[tauri::command]
pub async fn snippets_paste(app: AppHandle, state: State<'_, AppState>, id: String, values: Option<HashMap<String, String>>) -> AppResult<()> {
    require(&state, SNIPPETS)?;
    permissions::require(&state.db, PERM_PASTE, "snippets").await?;
    let delay = registry::settings(&state.db, SNIPPETS).await?["paste_delay_ms"].as_u64().unwrap_or(250);
    let text = use_snippet(&state.db, &id, &values.unwrap_or_default()).await?;
    crate::widgets::clipboard::copy(text).await?;
    crate::window::hide(&app);
    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    tauri::async_runtime::spawn_blocking(paste_keys).await.map_err(anyhow::Error::from)?;
    Ok(())
}

#[cfg(windows)]
pub(crate) fn paste_keys() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL};
    let key = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } },
    };
    const V: u16 = 0x56;
    let inputs = [key(VK_CONTROL, false), key(V, false), key(V, true), key(VK_CONTROL, true)];
    // SAFETY: a valid array of keyboard inputs.
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
}

#[cfg(not(windows))]
pub(crate) fn paste_keys() {}

#[tauri::command]
pub async fn snippets_export(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<String>> {
    require(&state, SNIPPETS)?;
    let json = export_json(&state.db).await?;
    let Some(path) = pick_save_path(&app, "Экспорт сниппетов", "focusdeck-snippets.json".into(), ("JSON", &["json"])).await? else {
        return Ok(None);
    };
    std::fs::write(&path, json).map_err(anyhow::Error::from)?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub async fn snippets_import(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<ImportReport>> {
    require(&state, SNIPPETS)?;
    let Some(json) = pick_open_text(&app, "Импорт сниппетов", ("JSON", &["json"]), 5 * 1024 * 1024).await? else { return Ok(None) };
    Ok(Some(import_json(&state.db, &json).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_and_render() {
        let body = "Здравствуйте, {{ имя }}! {{date}} {{имя}} {{ }} {{незакрыто";
        assert_eq!(variables(body), ["имя", "date"]);
        let values = HashMap::from([("имя".to_string(), "Анна".to_string())]);
        let out = render(body, &values);
        assert!(out.starts_with("Здравствуйте, Анна! "));
        assert!(out.ends_with(" Анна  {{незакрыто"));
        assert!(!out.contains("{{date}}"));
    }

    #[tokio::test]
    async fn crud_search_and_json_round_trip() {
        let pool = crate::db::test_pool().await;
        let s = save(&pool, None, SnippetInput { title: "Подпись".into(), body: "С уважением, {{имя}}".into(), shortcut: Some("sig".into()), tags: vec!["Почта".into()] }).await.unwrap();
        assert_eq!(s.variables, ["имя"]);
        assert_eq!(list(&pool, Some("SIG"), None).await.unwrap().len(), 1);
        assert_eq!(list(&pool, None, Some("почта")).await.unwrap().len(), 1);
        assert!(save(&pool, None, SnippetInput { title: "".into(), body: "x".into(), ..Default::default() }).await.is_err());

        let text = use_snippet(&pool, &s.id, &HashMap::from([("имя".into(), "Иван".into())])).await.unwrap();
        assert_eq!(text, "С уважением, Иван");
        assert_eq!(get(&pool, &s.id).await.unwrap().use_count, 1);

        let json = export_json(&pool).await.unwrap();
        let again = import_json(&pool, &json).await.unwrap();
        assert_eq!((again.imported, again.duplicates), (0, 1));
        delete(&pool, &s.id).await.unwrap();
        assert_eq!(import_json(&pool, &json).await.unwrap().imported, 1);
        assert!(import_json(&pool, "{\"format\":\"other\"}").await.is_err());
    }
}
