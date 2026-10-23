//! Data export / import: one JSON file with the chosen sections — settings
//! (app settings, modules, permissions via `modules::config`), layouts with
//! widgets, notes, tasks, calendar, snippets, automations.
//!
//! Rows are exported as they are stored. Import upserts by primary key:
//! existing rows with the same id are replaced, others are kept. Only
//! whitelisted tables are touched and only columns that exist in the current
//! schema are written (a file from an older version imports fine; a file from
//! a newer schema is refused). Secrets are never part of an export.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Column, Row, SqlitePool, TypeInfo, ValueRef};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub const FORMAT: &str = "focusdeck-data";
pub const VERSION: u32 = 1;
const MAX_FILE: u64 = 64 * 1024 * 1024;

/// Section → tables in dependency order (parents first).
pub const SECTIONS: &[(&str, &[&str])] = &[
    ("layouts", &["widget_layouts", "widgets"]),
    ("notes", &["notes"]),
    ("tasks", &["tasks"]),
    ("calendar", &["calendar_events"]),
    ("snippets", &["snippets"]),
    ("automations", &["automations"]),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFile {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub exported_at: String,
    /// Highest applied migration of the exporting database.
    #[serde(default)]
    pub schema: i64,
    /// Settings, modules and permissions (`modules::config` format).
    #[serde(default)]
    pub config: Option<Value>,
    /// table → rows
    #[serde(default)]
    pub tables: BTreeMap<String, Vec<Map<String, Value>>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportSummary {
    /// table → rows written
    pub imported: BTreeMap<String, usize>,
    /// table → rows skipped (constraint errors)
    pub skipped: BTreeMap<String, usize>,
    pub config_applied: bool,
    pub warnings: Vec<String>,
}

fn table_allowed(t: &str) -> bool {
    SECTIONS.iter().any(|(_, tables)| tables.contains(&t))
}

pub async fn schema_version(pool: &SqlitePool) -> AppResult<i64> {
    let (v,): (i64,) = sqlx::query_as("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations").fetch_one(pool).await?;
    Ok(v)
}

async fn columns(pool: &SqlitePool, table: &str) -> AppResult<Vec<String>> {
    debug_assert!(table_allowed(table));
    let sql = format!("SELECT name FROM pragma_table_info('{table}')");
    let rows: Vec<(String,)> = sqlx::query_as(sqlx::AssertSqlSafe(sql)).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn dump_table(pool: &SqlitePool, table: &str) -> AppResult<Vec<Map<String, Value>>> {
    if !table_allowed(table) {
        return Err(AppError::Invalid(format!("таблица {table} не экспортируется")));
    }
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT * FROM {table}"))).fetch_all(pool).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut obj = Map::new();
        for (i, col) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(i)?;
            let v = if raw.is_null() {
                Value::Null
            } else {
                match raw.type_info().name() {
                    "INTEGER" | "BOOLEAN" => Value::from(row.try_get::<i64, _>(i)?),
                    "REAL" => Value::from(row.try_get::<f64, _>(i)?),
                    "BLOB" => Value::Null,
                    _ => Value::from(row.try_get::<String, _>(i)?),
                }
            };
            obj.insert(col.name().to_string(), v);
        }
        out.push(obj);
    }
    Ok(out)
}

pub async fn export(pool: &SqlitePool, sections: &[String], config: Option<Value>, app_version: &str) -> AppResult<DataFile> {
    let mut tables = BTreeMap::new();
    for (name, list) in SECTIONS {
        if sections.iter().any(|s| s == name) {
            for t in *list {
                tables.insert(t.to_string(), dump_table(pool, t).await?);
            }
        }
    }
    Ok(DataFile {
        format: FORMAT.into(),
        version: VERSION,
        app_version: app_version.into(),
        exported_at: chrono::Local::now().to_rfc3339(),
        schema: schema_version(pool).await?,
        config,
        tables,
    })
}

pub fn parse(json: &str) -> AppResult<DataFile> {
    let f: DataFile = serde_json::from_str(json).map_err(|e| AppError::Invalid(format!("это не файл данных FocusDeck: {e}")))?;
    if f.format != FORMAT {
        return Err(AppError::Invalid("это не файл данных FocusDeck".into()));
    }
    if f.version > VERSION {
        return Err(AppError::Invalid("файл создан более новой версией FocusDeck".into()));
    }
    if let Some(t) = f.tables.keys().find(|t| !table_allowed(t)) {
        return Err(AppError::Invalid(format!("недопустимая таблица в файле: {t}")));
    }
    Ok(f)
}

fn bind_value<'q>(q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>, v: &Value) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    match v {
        Value::Null => q.bind(None::<String>),
        Value::Bool(b) => q.bind(*b),
        Value::Number(n) if n.is_i64() => q.bind(n.as_i64()),
        Value::Number(n) => q.bind(n.as_f64()),
        Value::String(s) => q.bind(s.clone()),
        other => q.bind(other.to_string()),
    }
}

/// Upserts the file's rows. Each table is one transaction; rows that violate
/// constraints are skipped and counted.
pub async fn import_tables(pool: &SqlitePool, file: &DataFile) -> AppResult<ImportSummary> {
    if file.schema > schema_version(pool).await? {
        return Err(AppError::Invalid("файл создан более новой версией FocusDeck — обновите приложение".into()));
    }
    let mut summary = ImportSummary::default();
    let order: Vec<&str> = SECTIONS.iter().flat_map(|(_, t)| t.iter().copied()).collect();
    for table in order {
        let Some(rows) = file.tables.get(table) else { continue };
        let cols = columns(pool, table).await?;
        let mut tx = pool.begin().await?;
        let (mut ok, mut bad) = (0usize, 0usize);
        for row in rows {
            let keys: Vec<&String> = row.keys().filter(|k| cols.contains(k)).collect();
            if !keys.contains(&&"id".to_string()) {
                bad += 1;
                continue;
            }
            let names = keys.iter().map(|k| format!("\"{k}\"")).collect::<Vec<_>>().join(", ");
            let marks = vec!["?"; keys.len()].join(", ");
            let updates = keys.iter().filter(|k| k.as_str() != "id").map(|k| format!("\"{k}\" = excluded.\"{k}\"")).collect::<Vec<_>>().join(", ");
            let sql = if updates.is_empty() {
                format!("INSERT OR IGNORE INTO {table} ({names}) VALUES ({marks})")
            } else {
                format!("INSERT INTO {table} ({names}) VALUES ({marks}) ON CONFLICT(id) DO UPDATE SET {updates}")
            };
            let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
            for k in &keys {
                // One active layout at most: imported layouts arrive inactive.
                let v = if table == "widget_layouts" && k.as_str() == "is_active" { &Value::from(0) } else { &row[k.as_str()] };
                q = bind_value(q, v);
            }
            // A savepoint per row: a failed row doesn't abort the table.
            sqlx::query("SAVEPOINT row").execute(&mut *tx).await?;
            match q.execute(&mut *tx).await {
                Ok(_) => {
                    sqlx::query("RELEASE row").execute(&mut *tx).await?;
                    ok += 1;
                }
                Err(e) => {
                    sqlx::query("ROLLBACK TO row").execute(&mut *tx).await?;
                    sqlx::query("RELEASE row").execute(&mut *tx).await?;
                    tracing::warn!(table, "import row skipped: {e}");
                    bad += 1;
                }
            }
        }
        tx.commit().await?;
        summary.imported.insert(table.into(), ok);
        if bad > 0 {
            summary.skipped.insert(table.into(), bad);
        }
    }
    // Make sure some layout is active.
    let (active,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM widget_layouts WHERE is_active = 1").fetch_one(pool).await?;
    if active == 0 {
        sqlx::query("UPDATE widget_layouts SET is_active = 1 WHERE id = (SELECT id FROM widget_layouts ORDER BY position LIMIT 1)").execute(pool).await?;
    }
    Ok(summary)
}

// --- commands ---

#[tauri::command]
pub async fn data_export(app: AppHandle, sections: Vec<String>, include_config: bool) -> AppResult<Option<String>> {
    let state = app.state::<AppState>();
    if let Some(bad) = sections.iter().find(|s| !SECTIONS.iter().any(|(n, _)| n == s)) {
        return Err(AppError::Invalid(format!("неизвестный раздел {bad}")));
    }
    if sections.is_empty() && !include_config {
        return Err(AppError::Invalid("выберите хотя бы один раздел".into()));
    }
    let version = app.package_info().version.to_string();
    let config = if include_config {
        Some(serde_json::to_value(crate::modules::config::export(&state.db, &state.settings(), &version).await?)?)
    } else {
        None
    };
    let file = export(&state.db, &sections, config, &version).await?;
    let name = format!("focusdeck-data-{}.json", chrono::Local::now().format("%Y%m%d-%H%M"));
    let Some(path) = crate::productivity::pick_save_path(&app, "Экспорт данных FocusDeck", name, ("JSON", &["json"])).await? else {
        return Ok(None);
    };
    std::fs::write(&path, serde_json::to_string_pretty(&file)?).map_err(anyhow::Error::from)?;
    tracing::info!(path = %path.display(), ?sections, include_config, "data exported");
    Ok(Some(path.display().to_string()))
}

/// Picks a data file and imports it (needs `confirm`: rows with the same id are replaced).
#[tauri::command]
pub async fn data_import(app: AppHandle, confirm: bool) -> AppResult<Option<ImportSummary>> {
    if !confirm {
        return Err(AppError::ConfirmationRequired(
            "Записи из файла будут добавлены; записи с теми же идентификаторами — заменены. Настройки, модули и разрешения из файла применятся. Продолжить?".into(),
        ));
    }
    let Some(json) = crate::productivity::pick_open_text(&app, "Импорт данных FocusDeck", ("JSON", &["json"]), MAX_FILE).await? else {
        return Ok(None);
    };
    let file = parse(&json)?;
    let state = app.state::<AppState>();
    let mut summary = import_tables(&state.db, &file).await?;
    if let Some(cfg) = &file.config {
        // Same path as the Control Center import: effects, hotkeys, logs.
        match crate::modules::import_config(app.clone(), app.state(), cfg.to_string(), Some(true)).await {
            Ok(r) => {
                summary.config_applied = true;
                summary.warnings.extend(r.warnings);
            }
            Err(e) => summary.warnings.push(format!("настройки: {e}")),
        }
    }
    { use tauri::Emitter; let _ = app.emit(crate::workspace::LAYOUT_EVENT, ()); }
    tracing::info!(?summary.imported, "data imported");
    Ok(Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn export_import_round_trip() {
        let src = crate::db::test_pool().await;
        crate::workspace::seed(&src).await.unwrap();
        sqlx::query("INSERT INTO notes (id, title, body, tags_json) VALUES ('n1', 'Заметка', 'текст', '[\"a\"]')").execute(&src).await.unwrap();
        sqlx::query("INSERT INTO tasks (id, title, priority) VALUES ('t1', 'Задача', 2)").execute(&src).await.unwrap();
        let all: Vec<String> = SECTIONS.iter().map(|(n, _)| n.to_string()).collect();
        let file = export(&src, &all, None, "test").await.unwrap();
        assert!(file.tables["notes"].iter().any(|r| r["id"] == "n1"));
        let json = serde_json::to_string(&file).unwrap();

        let dst = crate::db::test_pool().await;
        crate::workspace::seed(&dst).await.unwrap();
        sqlx::query("INSERT INTO notes (id, title) VALUES ('n1', 'старое')").execute(&dst).await.unwrap();
        let s = import_tables(&dst, &parse(&json).unwrap()).await.unwrap();
        assert_eq!(s.imported["notes"], file.tables["notes"].len());
        let (title, tags): (String, String) = sqlx::query_as("SELECT title, tags_json FROM notes WHERE id = 'n1'").fetch_one(&dst).await.unwrap();
        assert_eq!((title.as_str(), tags.as_str()), ("Заметка", "[\"a\"]"));
        let (p,): (i64,) = sqlx::query_as("SELECT priority FROM tasks WHERE id = 't1'").fetch_one(&dst).await.unwrap();
        assert_eq!(p, 2);
        let (active,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM widget_layouts WHERE is_active = 1").fetch_one(&dst).await.unwrap();
        assert_eq!(active, 1);
        // Importing twice changes nothing.
        let count = || async { sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM notes").fetch_one(&dst).await.unwrap().0 };
        let before = count().await;
        import_tables(&dst, &parse(&json).unwrap()).await.unwrap();
        assert_eq!(count().await, before);
    }

    #[tokio::test]
    async fn hostile_files_are_rejected() {
        assert!(parse("{}").is_err());
        assert!(parse(r#"{"format":"focusdeck-data","version":1,"tables":{"permissions":[]}}"#).is_err());
        assert!(parse(r#"{"format":"focusdeck-data","version":99}"#).is_err());
        let pool = crate::db::test_pool().await;
        let mut f = parse(r#"{"format":"focusdeck-data","version":1}"#).unwrap();
        f.schema = 999_999;
        assert!(import_tables(&pool, &f).await.is_err());
        // Unknown and injected column names are ignored, never reach SQL.
        let f = parse(r#"{"format":"focusdeck-data","version":1,"tables":{"notes":[{"id":"x","title":"t","evil\"); DROP TABLE notes; --":1}]}}"#).unwrap();
        let s = import_tables(&pool, &f).await.unwrap();
        assert_eq!(s.imported["notes"], 1);
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes").fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1);
    }
}
