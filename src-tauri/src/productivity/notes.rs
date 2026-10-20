//! Notes: Markdown text, tags, search, pinning, trash, export, and an
//! optional binding to a project name or a layout.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use super::{normalize_tags, parse_tags, pick_save_path, require, safe_file_name};
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::{catalog::NOTES, registry};
use crate::state::AppState;

const MAX_BODY: usize = 200_000;

#[derive(sqlx::FromRow)]
struct NoteRow {
    id: String,
    title: String,
    body: String,
    pinned: bool,
    tags_json: String,
    project: String,
    layout_id: Option<String>,
    deleted_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub project: String,
    pub layout_id: Option<String>,
    /// Set while the note is in the trash.
    pub deleted_at: Option<String>,
    /// Belongs to a «Заметки» widget on the dashboard.
    pub widget: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct NoteFilter {
    pub search: Option<String>,
    pub tag: Option<String>,
    pub project: Option<String>,
    pub layout_id: Option<String>,
    /// List the trash instead of the notes.
    #[serde(default)]
    pub trash: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct NoteInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub pinned: Option<bool>,
    pub project: Option<String>,
    /// `Some(None)` unbinds; omitted keeps the current layout.
    #[serde(default, deserialize_with = "double_option")]
    pub layout_id: Option<Option<String>>,
}

fn double_option<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(Option::<String>::deserialize(d)?))
}

async fn widget_note_ids(pool: &SqlitePool) -> AppResult<Vec<String>> {
    let rows: Vec<(Option<String>,)> = sqlx::query_as("SELECT json_extract(config_json, '$.note_id') FROM widgets WHERE kind = 'notes'")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().filter_map(|r| r.0).collect())
}

fn to_note(r: NoteRow, widget_ids: &[String]) -> Note {
    Note {
        widget: widget_ids.contains(&r.id),
        tags: parse_tags(&r.tags_json),
        id: r.id,
        title: r.title,
        body: r.body,
        pinned: r.pinned,
        project: r.project,
        layout_id: r.layout_id,
        deleted_at: r.deleted_at,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }
}

pub async fn list(pool: &SqlitePool, f: &NoteFilter) -> AppResult<Vec<Note>> {
    let rows = sqlx::query_as::<_, NoteRow>(
        "SELECT id, title, body, pinned, tags_json, project, layout_id, deleted_at, created_at, updated_at FROM notes
         WHERE (deleted_at IS NOT NULL) = ?
         ORDER BY pinned DESC, updated_at DESC",
    )
    .bind(f.trash)
    .fetch_all(pool)
    .await?;
    let widget_ids = widget_note_ids(pool).await?;
    let search = f.search.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    let tag = f.tag.as_deref().map(str::to_lowercase);
    Ok(rows
        .into_iter()
        .map(|r| to_note(r, &widget_ids))
        .filter(|n| search.as_ref().is_none_or(|s| n.title.to_lowercase().contains(s) || n.body.to_lowercase().contains(s) || n.tags.iter().any(|t| t.contains(s))))
        .filter(|n| tag.as_ref().is_none_or(|t| n.tags.contains(t)))
        .filter(|n| f.project.as_ref().is_none_or(|p| &n.project == p))
        .filter(|n| f.layout_id.as_ref().is_none_or(|l| n.layout_id.as_ref() == Some(l)))
        .collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Note> {
    let row = sqlx::query_as::<_, NoteRow>("SELECT id, title, body, pinned, tags_json, project, layout_id, deleted_at, created_at, updated_at FROM notes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("заметка".into()))?;
    Ok(to_note(row, &widget_note_ids(pool).await?))
}

fn check(input: &NoteInput) -> AppResult<()> {
    if input.title.as_ref().is_some_and(|t| t.chars().count() > 200) {
        return Err(AppError::Invalid("заголовок — не больше 200 символов".into()));
    }
    if input.body.as_ref().is_some_and(|b| b.chars().count() > MAX_BODY) {
        return Err(AppError::Invalid("заметка — не больше 200 000 символов".into()));
    }
    if input.project.as_ref().is_some_and(|p| p.chars().count() > 60) {
        return Err(AppError::Invalid("название проекта — не больше 60 символов".into()));
    }
    Ok(())
}

pub async fn create(pool: &SqlitePool, input: NoteInput) -> AppResult<Note> {
    check(&input)?;
    let id = new_id();
    let tags = normalize_tags(&input.tags.clone().unwrap_or_default())?;
    sqlx::query("INSERT INTO notes (id, title, body, pinned, tags_json, project, layout_id) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(input.title.as_deref().unwrap_or("").trim())
        .bind(input.body.as_deref().unwrap_or(""))
        .bind(input.pinned.unwrap_or(false))
        .bind(serde_json::to_string(&tags)?)
        .bind(input.project.as_deref().unwrap_or("").trim())
        .bind(input.layout_id.flatten())
        .execute(pool)
        .await?;
    get(pool, &id).await
}

pub async fn update(pool: &SqlitePool, id: &str, input: NoteInput) -> AppResult<Note> {
    check(&input)?;
    let current = get(pool, id).await?;
    let tags = input.tags.as_ref().map(|t| normalize_tags(t)).transpose()?.unwrap_or(current.tags);
    let layout = match input.layout_id {
        Some(l) => l,
        None => current.layout_id,
    };
    sqlx::query(
        "UPDATE notes SET title = ?, body = ?, pinned = ?, tags_json = ?, project = ?, layout_id = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(input.title.as_deref().map(str::trim).unwrap_or(&current.title))
    .bind(input.body.as_deref().unwrap_or(&current.body))
    .bind(input.pinned.unwrap_or(current.pinned))
    .bind(serde_json::to_string(&tags)?)
    .bind(input.project.as_deref().map(str::trim).unwrap_or(&current.project))
    .bind(layout)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn trash(pool: &SqlitePool, id: &str) -> AppResult<()> {
    if get(pool, id).await?.widget {
        return Err(AppError::Invalid("это заметка виджета: удалите сам виджет с обзора".into()));
    }
    sqlx::query("UPDATE notes SET deleted_at = datetime('now') WHERE id = ? AND deleted_at IS NULL").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn restore(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE notes SET deleted_at = NULL, updated_at = datetime('now') WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

/// Only notes already in the trash can be deleted for good.
pub async fn delete_forever(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let n = sqlx::query("DELETE FROM notes WHERE id = ? AND deleted_at IS NOT NULL").bind(id).execute(pool).await?.rows_affected();
    if n == 0 {
        return Err(AppError::Invalid("удалить навсегда можно только заметку из корзины".into()));
    }
    Ok(())
}

pub async fn empty_trash(pool: &SqlitePool) -> AppResult<u64> {
    Ok(sqlx::query("DELETE FROM notes WHERE deleted_at IS NOT NULL").execute(pool).await?.rows_affected())
}

/// Removes trashed notes older than the module's `trash_days`.
pub async fn purge_old(pool: &SqlitePool) -> AppResult<u64> {
    let days = registry::settings(pool, NOTES).await.ok().and_then(|s| s.get("trash_days").and_then(Value::as_i64)).unwrap_or(30);
    Ok(sqlx::query("DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < datetime('now', ?)")
        .bind(format!("-{days} days"))
        .execute(pool)
        .await?
        .rows_affected())
}

#[derive(Debug, Serialize)]
pub struct NoteFacets {
    pub tags: Vec<(String, i64)>,
    pub projects: Vec<String>,
    pub trash: i64,
}

pub async fn facets(pool: &SqlitePool) -> AppResult<NoteFacets> {
    let notes = list(pool, &NoteFilter::default()).await?;
    let mut tags: Vec<(String, i64)> = Vec::new();
    for t in notes.iter().flat_map(|n| n.tags.iter()) {
        match tags.iter_mut().find(|(n, _)| n == t) {
            Some(e) => e.1 += 1,
            None => tags.push((t.clone(), 1)),
        }
    }
    tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut projects: Vec<String> = notes.iter().map(|n| n.project.clone()).filter(|p| !p.is_empty()).collect();
    projects.sort();
    projects.dedup();
    let (trash,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes WHERE deleted_at IS NOT NULL").fetch_one(pool).await?;
    Ok(NoteFacets { tags, projects, trash })
}

/// Text of an exported note: Markdown keeps the source, plain text drops markup.
pub fn export_text(note: &Note, format: &str) -> String {
    let title = if note.title.is_empty() { "Без названия" } else { &note.title };
    if format == "md" {
        let mut out = format!("# {title}\n\n");
        if !note.tags.is_empty() {
            out += &format!("{}\n\n", note.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" "));
        }
        out + &note.body
    } else {
        format!("{title}\n\n{}", strip_markdown(&note.body))
    }
}

/// Simple Markdown → text: headings, emphasis, code fences, links, list markers.
pub fn strip_markdown(md: &str) -> String {
    md.lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .map(|l| {
            let mut s = l.trim_start_matches('#').trim_start_matches('>').to_string();
            if l.starts_with('#') || l.starts_with('>') {
                s = s.trim_start().to_string();
            }
            for m in ["**", "__", "`", "~~"] {
                s = s.replace(m, "");
            }
            // [text](url) → text (url)
            while let (Some(a), Some(b)) = (s.find('['), s.find("](")) {
                let Some(c) = s[b..].find(')').map(|c| b + c) else { break };
                if a > b {
                    break;
                }
                s = format!("{}{} ({}){}", &s[..a], &s[a + 1..b], &s[b + 2..c], &s[c + 1..]);
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// --- commands ---

#[tauri::command]
pub async fn notes_list(state: State<'_, AppState>, filter: Option<NoteFilter>) -> AppResult<Vec<Note>> {
    require(&state, NOTES)?;
    purge_old(&state.db).await?;
    list(&state.db, &filter.unwrap_or_default()).await
}

#[tauri::command]
pub async fn notes_facets(state: State<'_, AppState>) -> AppResult<NoteFacets> {
    require(&state, NOTES)?;
    facets(&state.db).await
}

#[tauri::command]
pub async fn notes_create(state: State<'_, AppState>, input: NoteInput) -> AppResult<Note> {
    require(&state, NOTES)?;
    create(&state.db, input).await
}

#[tauri::command]
pub async fn notes_update(state: State<'_, AppState>, id: String, input: NoteInput) -> AppResult<Note> {
    require(&state, NOTES)?;
    update(&state.db, &id, input).await
}

#[tauri::command]
pub async fn notes_trash(state: State<'_, AppState>, id: String) -> AppResult<()> {
    require(&state, NOTES)?;
    trash(&state.db, &id).await
}

#[tauri::command]
pub async fn notes_restore(state: State<'_, AppState>, id: String) -> AppResult<()> {
    require(&state, NOTES)?;
    restore(&state.db, &id).await
}

#[tauri::command]
pub async fn notes_delete_forever(state: State<'_, AppState>, id: String) -> AppResult<()> {
    require(&state, NOTES)?;
    delete_forever(&state.db, &id).await
}

#[tauri::command]
pub async fn notes_empty_trash(state: State<'_, AppState>) -> AppResult<u64> {
    require(&state, NOTES)?;
    empty_trash(&state.db).await
}

/// Saves a note as `.md` or `.txt` through a «Save as» dialog; returns the path or `null`.
#[tauri::command]
pub async fn notes_export(app: AppHandle, state: State<'_, AppState>, id: String, format: String) -> AppResult<Option<String>> {
    require(&state, NOTES)?;
    let (ext, filter): (&str, (&'static str, &'static [&'static str])) = match format.as_str() {
        "md" => ("md", ("Markdown", &["md"])),
        "txt" => ("txt", ("Текст", &["txt"])),
        other => return Err(AppError::Invalid(format!("формат {other} не поддерживается"))),
    };
    let note = get(&state.db, &id).await?;
    let name = format!("{}.{ext}", safe_file_name(&note.title, "note"));
    let Some(path) = pick_save_path(&app, "Экспорт заметки", name, filter).await? else { return Ok(None) };
    std::fs::write(&path, export_text(&note, &format)).map_err(anyhow::Error::from)?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(title: &str, body: &str, tags: &[&str]) -> NoteInput {
        NoteInput { title: Some(title.into()), body: Some(body.into()), tags: Some(tags.iter().map(|t| t.to_string()).collect()), ..Default::default() }
    }

    #[tokio::test]
    async fn crud_search_tags_and_trash() {
        let pool = crate::db::test_pool().await;
        let a = create(&pool, input("План", "# Неделя\n- **отчёт**", &["#Работа"])).await.unwrap();
        let b = create(&pool, input("Покупки", "молоко", &["дом"])).await.unwrap();
        assert_eq!(a.tags, ["работа"]);

        let f = |search: &str| NoteFilter { search: Some(search.into()), ..Default::default() };
        assert_eq!(list(&pool, &f("отчёт")).await.unwrap().len(), 1);
        let by_tag = NoteFilter { tag: Some("дом".into()), ..Default::default() };
        assert_eq!(list(&pool, &by_tag).await.unwrap()[0].id, b.id);

        update(&pool, &b.id, NoteInput { pinned: Some(true), project: Some("Дом".into()), ..Default::default() }).await.unwrap();
        assert_eq!(list(&pool, &NoteFilter::default()).await.unwrap()[0].id, b.id, "pinned first");

        assert!(delete_forever(&pool, &a.id).await.is_err(), "only from the trash");
        trash(&pool, &a.id).await.unwrap();
        assert_eq!(list(&pool, &NoteFilter::default()).await.unwrap().len(), 1);
        assert_eq!(list(&pool, &NoteFilter { trash: true, ..Default::default() }).await.unwrap().len(), 1);
        restore(&pool, &a.id).await.unwrap();
        trash(&pool, &a.id).await.unwrap();
        assert_eq!(empty_trash(&pool).await.unwrap(), 1);

        let facets = facets(&pool).await.unwrap();
        assert_eq!(facets.tags, [("дом".to_string(), 1)]);
        assert_eq!(facets.projects, ["Дом"]);
    }

    #[test]
    fn export_formats() {
        let note = Note {
            id: "1".into(),
            title: "План".into(),
            body: "## Шаг\n**важно** и [сайт](https://example.com)\n```\ncode\n```".into(),
            pinned: false,
            tags: vec!["работа".into()],
            project: String::new(),
            layout_id: None,
            deleted_at: None,
            widget: false,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!(export_text(&note, "md").starts_with("# План\n\n#работа\n\n## Шаг"));
        assert_eq!(export_text(&note, "txt"), "План\n\nШаг\nважно и сайт (https://example.com)\ncode");
    }
}
