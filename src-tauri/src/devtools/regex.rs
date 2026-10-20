//! Regex Tester: saved expressions. Matching itself runs in the UI with the
//! JavaScript engine (in a worker with a timeout), so what you test is what
//! a browser or Node.js would do.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use super::require;
use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::modules::catalog::DEV_REGEX;
use crate::state::AppState;

/// JavaScript RegExp flags.
pub const FLAGS: &str = "dgimsuvy";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SavedRegex {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub flags: String,
    pub sample: String,
    pub replacement: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegexInput {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub flags: String,
    #[serde(default)]
    pub sample: String,
    #[serde(default)]
    pub replacement: String,
}

pub fn validate(i: &RegexInput) -> AppResult<()> {
    let name = i.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(AppError::Invalid("название: от 1 до 100 символов".into()));
    }
    if i.pattern.is_empty() || i.pattern.len() > 10_000 {
        return Err(AppError::Invalid("выражение: от 1 до 10 000 символов".into()));
    }
    let mut seen = String::new();
    for c in i.flags.chars() {
        if !FLAGS.contains(c) || seen.contains(c) {
            return Err(AppError::Invalid(format!("недопустимый флаг «{c}»")));
        }
        seen.push(c);
    }
    if i.flags.contains('u') && i.flags.contains('v') {
        return Err(AppError::Invalid("флаги u и v нельзя использовать вместе".into()));
    }
    if i.sample.len() > 1024 * 1024 || i.replacement.len() > 10_000 {
        return Err(AppError::Invalid("пример текста или замена слишком большие".into()));
    }
    Ok(())
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<SavedRegex>> {
    Ok(sqlx::query_as::<_, SavedRegex>("SELECT id, name, pattern, flags, sample, replacement, updated_at FROM regex_saved ORDER BY name").fetch_all(pool).await?)
}

pub async fn save(pool: &SqlitePool, id: Option<&str>, i: RegexInput) -> AppResult<SavedRegex> {
    validate(&i)?;
    let id = match id {
        Some(id) => {
            let n = sqlx::query("UPDATE regex_saved SET name = ?, pattern = ?, flags = ?, sample = ?, replacement = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(i.name.trim())
                .bind(&i.pattern)
                .bind(&i.flags)
                .bind(&i.sample)
                .bind(&i.replacement)
                .bind(id)
                .execute(pool)
                .await?
                .rows_affected();
            if n == 0 {
                return Err(AppError::NotFound("выражение".into()));
            }
            id.to_string()
        }
        None => {
            let id = new_id();
            sqlx::query("INSERT INTO regex_saved (id, name, pattern, flags, sample, replacement) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(&id)
                .bind(i.name.trim())
                .bind(&i.pattern)
                .bind(&i.flags)
                .bind(&i.sample)
                .bind(&i.replacement)
                .execute(pool)
                .await?;
            id
        }
    };
    Ok(sqlx::query_as::<_, SavedRegex>("SELECT id, name, pattern, flags, sample, replacement, updated_at FROM regex_saved WHERE id = ?").bind(id).fetch_one(pool).await?)
}

#[tauri::command]
pub async fn regex_list(app: AppHandle) -> AppResult<Vec<SavedRegex>> {
    require(&app, DEV_REGEX)?;
    list(&app.state::<AppState>().db).await
}

#[tauri::command]
pub async fn regex_save(app: AppHandle, id: Option<String>, input: RegexInput) -> AppResult<SavedRegex> {
    require(&app, DEV_REGEX)?;
    save(&app.state::<AppState>().db, id.as_deref(), input).await
}

#[tauri::command]
pub async fn regex_delete(app: AppHandle, id: String) -> AppResult<()> {
    require(&app, DEV_REGEX)?;
    sqlx::query("DELETE FROM regex_saved WHERE id = ?").bind(id).execute(&app.state::<AppState>().db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(flags: &str) -> RegexInput {
        RegexInput { name: "Email".into(), pattern: r"\w+@\w+".into(), flags: flags.into(), sample: "a@b".into(), replacement: String::new() }
    }

    #[tokio::test]
    async fn save_and_validate() {
        assert!(validate(&input("gim")).is_ok());
        assert!(validate(&input("gg")).is_err());
        assert!(validate(&input("x")).is_err());
        assert!(validate(&input("uv")).is_err());
        let pool = crate::db::test_pool().await;
        let r = save(&pool, None, input("g")).await.unwrap();
        let r2 = save(&pool, Some(&r.id), RegexInput { name: " Почта ".into(), ..input("gi") }).await.unwrap();
        assert_eq!((r2.name.as_str(), r2.flags.as_str()), ("Почта", "gi"));
        assert!(save(&pool, Some("missing"), input("g")).await.is_err());
        assert_eq!(list(&pool).await.unwrap().len(), 1);
    }
}
