//! Permissions the user grants to app features. Every change is written to
//! `permission_logs`. Revoking a permission switches the feature off.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub const PERM_AUTOSTART: &str = "system.autostart";
pub const PERM_GLOBAL_HOTKEYS: &str = "system.global_hotkeys";
pub const PERM_NOTIFICATIONS: &str = "app.notifications";
pub const PERM_CLIPBOARD_READ: &str = "clipboard.read";
pub const PERM_FS_READ: &str = "fs.read";

struct Definition {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    granted_by_default: bool,
}

const DEFINITIONS: &[Definition] = &[
    Definition {
        id: PERM_GLOBAL_HOTKEYS,
        name: "Глобальные горячие клавиши",
        description: "Регистрировать сочетания клавиш, работающие во всех программах.",
        granted_by_default: true,
    },
    Definition {
        id: PERM_AUTOSTART,
        name: "Автозапуск",
        description: "Запускать FocusDeck вместе с Windows.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_NOTIFICATIONS,
        name: "Уведомления",
        description: "Показывать уведомления от автоматизаций внутри приложения.",
        granted_by_default: true,
    },
    Definition {
        id: PERM_CLIPBOARD_READ,
        name: "Чтение буфера обмена",
        description: "Читать буфер обмена для модуля «История буфера».",
        granted_by_default: false,
    },
    Definition {
        id: PERM_FS_READ,
        name: "Чтение файлов",
        description: "Показывать выбранные папки и файлы в модуле «Файлы».",
        granted_by_default: false,
    },
];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Permission {
    pub id: String,
    pub name: String,
    pub description: String,
    pub granted: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PermissionLog {
    pub id: i64,
    pub permission_id: String,
    pub action: String,
    pub source: String,
    pub created_at: String,
}

/// Inserts missing permissions with their defaults; existing choices are kept.
pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    for d in DEFINITIONS {
        sqlx::query(
            "INSERT INTO permissions (id, name, description, granted) VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description",
        )
        .bind(d.id)
        .bind(d.name)
        .bind(d.description)
        .bind(d.granted_by_default)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Permission>> {
    Ok(
        sqlx::query_as::<_, Permission>("SELECT * FROM permissions ORDER BY name")
            .fetch_all(pool)
            .await?,
    )
}

pub async fn is_granted(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let row: Option<(bool,)> = sqlx::query_as("SELECT granted FROM permissions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0).unwrap_or(false))
}

pub async fn log(pool: &SqlitePool, id: &str, action: &str, source: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO permission_logs (permission_id, action, source) VALUES (?, ?, ?)")
        .bind(id)
        .bind(action)
        .bind(source)
        .execute(pool)
        .await?;
    Ok(())
}

/// Sets a permission and logs the change. Returns whether anything changed.
pub async fn set(pool: &SqlitePool, id: &str, granted: bool, source: &str) -> AppResult<bool> {
    let current = sqlx::query_as::<_, (bool,)>("SELECT granted FROM permissions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("разрешение {id}")))?;
    if current.0 == granted {
        return Ok(false);
    }
    sqlx::query("UPDATE permissions SET granted = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(granted)
        .bind(id)
        .execute(pool)
        .await?;
    log(
        pool,
        id,
        if granted { "granted" } else { "revoked" },
        source,
    )
    .await?;
    tracing::info!(permission = id, granted, source, "permission changed");
    Ok(true)
}

pub async fn ensure_granted(pool: &SqlitePool, id: &str, source: &str) -> AppResult<()> {
    set(pool, id, true, source).await.map(|_| ())
}

/// Fails with `PermissionDenied` (and logs a denial) when not granted.
pub async fn require(pool: &SqlitePool, id: &str, source: &str) -> AppResult<()> {
    if is_granted(pool, id).await? {
        return Ok(());
    }
    // The Security module decides whether denials are logged (default: yes).
    let log_denials = crate::modules::registry::settings(pool, crate::modules::catalog::SECURITY)
        .await
        .ok()
        .and_then(|s| s.get("log_denials").and_then(serde_json::Value::as_bool))
        .unwrap_or(true);
    if log_denials {
        log(pool, id, "denied", source).await?;
    }
    let name = DEFINITIONS
        .iter()
        .find(|d| d.id == id)
        .map(|d| d.name)
        .unwrap_or(id);
    Err(AppError::PermissionDenied(format!(
        "«{name}» выключено на странице «Разрешения»"
    )))
}

#[tauri::command]
pub async fn permissions_list(state: State<'_, AppState>) -> AppResult<Vec<Permission>> {
    list(&state.db).await
}

/// Grants or revokes a permission; revoking turns off the dependent feature.
#[tauri::command]
pub async fn permissions_set(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    granted: bool,
) -> AppResult<Vec<Permission>> {
    if !state.module_enabled(crate::modules::catalog::SECURITY) {
        return Err(AppError::ModuleDisabled("Безопасность".into()));
    }
    set(&state.db, &id, granted, "user").await?;

    if !granted {
        let mut settings = state.settings();
        let mut changed = false;
        if id == PERM_AUTOSTART && settings.autostart {
            crate::autostart::set(&app, false)?;
            settings.autostart = false;
            changed = true;
        }
        if id == PERM_GLOBAL_HOTKEYS && settings.global_hotkeys_enabled {
            settings.global_hotkeys_enabled = false;
            changed = true;
        }
        if changed {
            crate::settings::save(&state.db, &settings).await?;
            *state.settings.write().unwrap() = settings;
        }
    }
    if id == PERM_GLOBAL_HOTKEYS {
        crate::hotkeys::sync(&app).await?;
    }
    // Modules show missing permissions: let the UI refresh.
    crate::modules::effects::notify_changed(&app);
    list(&state.db).await
}

#[tauri::command]
pub async fn permissions_logs(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<PermissionLog>> {
    Ok(
        sqlx::query_as::<_, PermissionLog>(
            "SELECT * FROM permission_logs ORDER BY id DESC LIMIT ?",
        )
        .bind(limit.unwrap_or(100).clamp(1, 1000))
        .fetch_all(&state.db)
        .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_set_require_and_log() {
        let pool = crate::db::test_pool().await;
        seed(&pool).await.unwrap();
        assert_eq!(list(&pool).await.unwrap().len(), DEFINITIONS.len());
        assert!(is_granted(&pool, PERM_GLOBAL_HOTKEYS).await.unwrap());
        assert!(!is_granted(&pool, PERM_AUTOSTART).await.unwrap());

        assert!(matches!(
            require(&pool, PERM_AUTOSTART, "test").await,
            Err(AppError::PermissionDenied(_))
        ));
        assert!(set(&pool, PERM_AUTOSTART, true, "user").await.unwrap());
        assert!(
            !set(&pool, PERM_AUTOSTART, true, "user").await.unwrap(),
            "no-op change is not logged"
        );
        require(&pool, PERM_AUTOSTART, "test").await.unwrap();

        // Re-seeding keeps the user's choice.
        seed(&pool).await.unwrap();
        assert!(is_granted(&pool, PERM_AUTOSTART).await.unwrap());

        let actions: Vec<(String,)> =
            sqlx::query_as("SELECT action FROM permission_logs ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            actions.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(),
            ["denied", "granted"]
        );
    }
}
