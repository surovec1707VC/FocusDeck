//! Built-in modules and their persisted state (`modules`, `module_settings`).

use serde_json::Value;
use sqlx::SqlitePool;

use super::types::{ModuleDef, ModuleInfo};
use crate::error::{AppError, AppResult};
use crate::permissions::{self, PERM_CLIPBOARD_READ, PERM_GLOBAL_HOTKEYS, PERM_NOTIFICATIONS};

pub const MODULE_AUTOMATIONS: &str = "automations";
pub const MODULE_WIDGETS: &str = "widgets";

pub const BUILTIN: &[ModuleDef] = &[
    ModuleDef {
        id: "dashboard",
        name: "Обзор",
        description: "Главная страница со сводкой состояния приложения.",
        category: "Основное",
        enabled_by_default: true,
        is_core: true,
        permissions: &[],
    },
    ModuleDef {
        id: MODULE_WIDGETS,
        name: "Виджеты и макеты",
        description: "Набор виджетов на рабочем экране и переключаемые макеты.",
        category: "Рабочее место",
        enabled_by_default: true,
        is_core: false,
        permissions: &[],
    },
    ModuleDef {
        id: MODULE_AUTOMATIONS,
        name: "Автоматизации",
        description: "Правила «триггер → действие»: по запуску, по расписанию, по горячей клавише.",
        category: "Автоматизация",
        enabled_by_default: true,
        is_core: false,
        permissions: &[PERM_NOTIFICATIONS],
    },
    ModuleDef {
        id: "hotkeys",
        name: "Горячие клавиши",
        description: "Глобальные сочетания клавиш для показа окна и запуска автоматизаций.",
        category: "Система",
        enabled_by_default: true,
        is_core: false,
        permissions: &[PERM_GLOBAL_HOTKEYS],
    },
    ModuleDef {
        id: "tasks",
        name: "Задачи",
        description: "Быстрый список задач (виджет «Задачи»).",
        category: "Рабочее место",
        enabled_by_default: true,
        is_core: false,
        permissions: &[],
    },
    ModuleDef {
        id: "notes",
        name: "Заметки",
        description: "Короткие заметки. Хранилище готово, интерфейс — в разработке.",
        category: "Рабочее место",
        enabled_by_default: false,
        is_core: false,
        permissions: &[],
    },
    ModuleDef {
        id: "snippets",
        name: "Сниппеты",
        description: "Шаблоны текста для быстрой вставки. Хранилище готово, интерфейс — в разработке.",
        category: "Рабочее место",
        enabled_by_default: false,
        is_core: false,
        permissions: &[],
    },
    ModuleDef {
        id: "clipboard_history",
        name: "История буфера обмена",
        description: "Локальная история скопированного текста. В разработке.",
        category: "Система",
        enabled_by_default: false,
        is_core: false,
        permissions: &[PERM_CLIPBOARD_READ],
    },
];

pub fn def(id: &str) -> AppResult<&'static ModuleDef> {
    BUILTIN.iter().find(|m| m.id == id).ok_or_else(|| AppError::NotFound(format!("модуль {id}")))
}

/// Inserts new built-in modules; keeps the user's enabled flag for known ones.
pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    for m in BUILTIN {
        sqlx::query(
            "INSERT INTO modules (id, name, description, category, enabled, is_core) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description,
                category = excluded.category, is_core = excluded.is_core",
        )
        .bind(m.id)
        .bind(m.name)
        .bind(m.description)
        .bind(m.category)
        .bind(m.enabled_by_default || m.is_core)
        .bind(m.is_core)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn is_enabled(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let row: Option<(bool,)> = sqlx::query_as("SELECT enabled FROM modules WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0).unwrap_or(false))
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<ModuleInfo>> {
    let mut out = Vec::with_capacity(BUILTIN.len());
    for m in BUILTIN {
        let mut missing = Vec::new();
        for p in m.permissions {
            if !permissions::is_granted(pool, p).await? {
                missing.push(p.to_string());
            }
        }
        out.push(ModuleInfo {
            id: m.id.into(),
            name: m.name.into(),
            description: m.description.into(),
            category: m.category.into(),
            enabled: is_enabled(pool, m.id).await?,
            is_core: m.is_core,
            permissions: m.permissions.iter().map(|p| p.to_string()).collect(),
            missing_permissions: missing,
        });
    }
    Ok(out)
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<()> {
    let m = def(id)?;
    if m.is_core && !enabled {
        return Err(AppError::Invalid(format!("модуль «{}» нельзя отключить", m.name)));
    }
    sqlx::query("UPDATE modules SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(enabled)
        .bind(id)
        .execute(pool)
        .await?;
    tracing::info!(module = id, enabled, "module toggled");
    Ok(())
}

pub async fn get_setting(pool: &SqlitePool, module_id: &str, key: &str) -> AppResult<Option<Value>> {
    def(module_id)?;
    let row: Option<(String,)> = sqlx::query_as("SELECT value_json FROM module_settings WHERE module_id = ? AND key = ?")
        .bind(module_id)
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(match row {
        Some((json,)) => Some(serde_json::from_str(&json)?),
        None => None,
    })
}

pub async fn set_setting(pool: &SqlitePool, module_id: &str, key: &str, value: &Value) -> AppResult<()> {
    def(module_id)?;
    sqlx::query(
        "INSERT INTO module_settings (module_id, key, value_json) VALUES (?, ?, ?)
         ON CONFLICT(module_id, key) DO UPDATE SET value_json = excluded.value_json, updated_at = datetime('now')",
    )
    .bind(module_id)
    .bind(key)
    .bind(value.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn pool() -> SqlitePool {
        let pool = crate::db::test_pool().await;
        permissions::seed(&pool).await.unwrap();
        seed(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn seed_list_toggle() {
        let pool = pool().await;
        let modules = list(&pool).await.unwrap();
        assert_eq!(modules.len(), BUILTIN.len());
        let clip = modules.iter().find(|m| m.id == "clipboard_history").unwrap();
        assert_eq!(clip.missing_permissions, [PERM_CLIPBOARD_READ]);

        set_enabled(&pool, "notes", true).await.unwrap();
        seed(&pool).await.unwrap();
        assert!(is_enabled(&pool, "notes").await.unwrap(), "re-seed keeps user choice");

        assert!(matches!(set_enabled(&pool, "dashboard", false).await, Err(AppError::Invalid(_))));
        assert!(matches!(set_enabled(&pool, "nope", true).await, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn module_settings_round_trip() {
        let pool = pool().await;
        assert_eq!(get_setting(&pool, "tasks", "sort").await.unwrap(), None);
        set_setting(&pool, "tasks", "sort", &json!("newest")).await.unwrap();
        assert_eq!(get_setting(&pool, "tasks", "sort").await.unwrap(), Some(json!("newest")));
    }
}
