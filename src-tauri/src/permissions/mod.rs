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
pub const PERM_NETWORK_WEATHER: &str = "network.weather";
pub const PERM_LAUNCH_APPS: &str = "system.launch_apps";
pub const PERM_PASTE: &str = "system.paste";
pub const PERM_RUN_COMMANDS: &str = "system.run_commands";
pub const PERM_CLOSE_APPS: &str = "system.close_apps";
pub const PERM_DELETE_FILES: &str = "files.delete";
pub const PERM_MOVE_FILES: &str = "files.move";
pub const PERM_CLIPBOARD_WRITE: &str = "clipboard.write";
pub const PERM_WINDOWS: &str = "windows.manage";
pub const PERM_KEYBOARD: &str = "input.keyboard";
pub const PERM_MOUSE: &str = "input.mouse";
pub const PERM_SCREEN: &str = "screen.capture";
pub const PERM_SYSTEM_SETTINGS: &str = "system.settings";
pub const PERM_UI_AUTOMATION: &str = "ui.automation";
pub const PERM_GIT: &str = "dev.git";
pub const PERM_NETWORK_API: &str = "network.api_tester";

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
        description: "Читать буфер обмена для модуля Clipboard Hub (текст, ссылки, файлы, изображения, HTML, RTF). Хранится только локально.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_FS_READ,
        name: "Чтение файлов",
        description: "Показывать выбранные папки и файлы в модуле «Файлы».",
        granted_by_default: false,
    },
    Definition {
        id: PERM_NETWORK_WEATHER,
        name: "Сеть: прогноз погоды",
        description: "Запрашивать погоду для выбранного города у open-meteo.com (без ключей и учётных \
                      записей). Это единственный сетевой запрос FocusDeck.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_LAUNCH_APPS,
        name: "Запуск программ",
        description: "Открывать программы и ярлыки, которые вы сами добавили в виджет «Запуск программ».",
        granted_by_default: false,
    },
    Definition {
        id: PERM_PASTE,
        name: "Вставка текста",
        description: "По кнопке «Вставить» (сниппеты, история буфера, быстрые действия) нажимать Ctrl+V в программе, где вы работали.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_RUN_COMMANDS,
        name: "Выполнение команд",
        description: "Запускать команды терминала из быстрых действий и автоматизаций. Каждый запуск подтверждается отдельно.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_CLOSE_APPS,
        name: "Закрытие программ",
        description: "Автоматизации могут закрывать программы по имени процесса. Каждый запуск подтверждается.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_DELETE_FILES,
        name: "Удаление файлов",
        description: "Автоматизации могут перемещать файлы в Корзину (например, очистка рабочего стола). Каждый запуск подтверждается.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_MOVE_FILES,
        name: "Перемещение файлов",
        description: "Desktop Organizer раскладывает файлы рабочего стола по папкам (только перемещение, с журналом и отменой).",
        granted_by_default: true,
    },
    Definition {
        id: PERM_CLIPBOARD_WRITE,
        name: "Запись в буфер обмена",
        description: "Помещать текст в буфер обмена по команде из Windows Integration. Прежнее содержимое можно вернуть отменой.",
        granted_by_default: true,
    },
    Definition {
        id: PERM_WINDOWS,
        name: "Управление окнами",
        description: "Видеть список открытых окон других программ, сворачивать, разворачивать, переключать и закрывать их.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_KEYBOARD,
        name: "Ввод с клавиатуры",
        description: "Нажимать сочетания клавиш от имени пользователя (переключение виртуальных рабочих столов Win+Ctrl+←/→).",
        granted_by_default: false,
    },
    Definition {
        id: PERM_MOUSE,
        name: "Ввод мышью",
        description: "Двигать курсор и нажимать кнопки мыши. Сейчас ни одна функция FocusDeck этого не делает.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_SCREEN,
        name: "Скриншоты",
        description: "Снимать изображение экрана (Windows Integration, действие автоматизаций «Сделать скриншот»). Снимки сохраняются локально.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_SYSTEM_SETTINGS,
        name: "Системные настройки",
        description: "Менять громкость, яркость экрана и схему электропитания. Изменения можно отменить в журнале действий.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_UI_AUTOMATION,
        name: "UI Automation",
        description: "Читать структуру интерфейса окон других программ (кнопки, поля, их названия) через Windows UI Automation. Только по явному запросу.",
        granted_by_default: false,
    },
    Definition {
        id: PERM_GIT,
        name: "Git: изменение репозиториев",
        description: "Developer Tools → Git: добавлять файлы в индекс, создавать коммиты, выполнять push и pull (push и pull — только после подтверждения).",
        granted_by_default: false,
    },
    Definition {
        id: PERM_NETWORK_API,
        name: "Сеть: API Tester",
        description: "Отправлять HTTP-запросы, которые вы сами составили в API Tester, на указанные вами адреса.",
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

/// Groups of the Permissions Center.
pub fn category(id: &str) -> &'static str {
    match id {
        PERM_FS_READ | PERM_DELETE_FILES | PERM_MOVE_FILES => "files",
        PERM_CLIPBOARD_READ | PERM_CLIPBOARD_WRITE => "clipboard",
        PERM_WINDOWS | PERM_UI_AUTOMATION => "windows",
        PERM_KEYBOARD | PERM_PASTE | PERM_GLOBAL_HOTKEYS => "keyboard",
        PERM_MOUSE => "mouse",
        PERM_SCREEN => "screen",
        PERM_LAUNCH_APPS | PERM_RUN_COMMANDS | PERM_CLOSE_APPS | PERM_AUTOSTART | PERM_GIT => "processes",
        PERM_NETWORK_WEATHER | PERM_NETWORK_API => "network",
        PERM_NOTIFICATIONS => "notifications",
        _ => "system",
    }
}

/// Modules that use a permission inside some of their actions (in addition
/// to the module catalog, which lists the permissions a module needs to work).
const ACTION_USES: &[(&str, &[&str])] = &[
    (PERM_LAUNCH_APPS, &["widget_app_launcher", "clipboard_history", "automations", "windows_integration", "dev_terminal"]),
    (PERM_PASTE, &["snippets", "clipboard_history", "automations"]),
    (PERM_RUN_COMMANDS, &["clipboard_history", "automations", "dev_projects"]),
    (PERM_GIT, &["dev_git"]),
    (PERM_CLOSE_APPS, &["automations"]),
    (PERM_DELETE_FILES, &["automations"]),
    (PERM_MOVE_FILES, &["desktop_organizer"]),
    (PERM_NOTIFICATIONS, &["automations", "timers", "windows_integration"]),
    (PERM_CLIPBOARD_READ, &["windows_integration"]),
    (PERM_CLIPBOARD_WRITE, &["windows_integration"]),
    (PERM_WINDOWS, &["windows_integration"]),
    (PERM_KEYBOARD, &["windows_integration"]),
    (PERM_SCREEN, &["windows_integration", "automations"]),
    (PERM_SYSTEM_SETTINGS, &["windows_integration"]),
    (PERM_UI_AUTOMATION, &["windows_integration"]),
];

#[derive(Debug, Clone, Serialize)]
pub struct ModuleRef {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionInfo {
    #[serde(flatten)]
    pub permission: Permission,
    pub category: &'static str,
    pub used_by: Vec<ModuleRef>,
    /// Uses recorded in the Action Log.
    pub uses: i64,
    pub last_used_at: Option<String>,
    pub denials: i64,
}

pub async fn center(pool: &SqlitePool) -> AppResult<Vec<PermissionInfo>> {
    let enabled = crate::modules::registry::enabled_set(pool).await?;
    let mut out = Vec::new();
    for p in list(pool).await? {
        let mut ids: Vec<&str> = crate::modules::catalog::BUILTIN.iter().filter(|m| m.permissions.contains(&p.id.as_str())).map(|m| m.id).collect();
        for (perm, mods) in ACTION_USES {
            if *perm == p.id {
                for m in mods.iter() {
                    if !ids.contains(m) {
                        ids.push(m);
                    }
                }
            }
        }
        let used_by = ids
            .into_iter()
            .filter_map(|id| crate::modules::catalog::find(id))
            .map(|m| ModuleRef { id: m.id.into(), name: m.name.into(), enabled: enabled.contains(m.id) })
            .collect();
        let (uses, last): (i64, Option<String>) = sqlx::query_as("SELECT COUNT(*), MAX(created_at) FROM action_log WHERE permission = ? AND result = 'ok'")
            .bind(&p.id)
            .fetch_one(pool)
            .await?;
        let (denials,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM permission_logs WHERE permission_id = ? AND action = 'denied'").bind(&p.id).fetch_one(pool).await?;
        out.push(PermissionInfo { category: category(&p.id), permission: p, used_by, uses, last_used_at: last, denials });
    }
    Ok(out)
}

/// One line of a permission's access log: grants/revocations/denials and uses.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AccessEntry {
    pub created_at: String,
    /// granted | revoked | denied | used | blocked | error
    pub kind: String,
    pub source: String,
    pub detail: String,
}

pub async fn access_log(pool: &SqlitePool, id: &str, limit: i64) -> AppResult<Vec<AccessEntry>> {
    Ok(sqlx::query_as::<_, AccessEntry>(
        "SELECT created_at, kind, source, detail FROM (
            SELECT created_at, action AS kind, source, '' AS detail, id * 2 AS ord FROM permission_logs WHERE permission_id = ?1
            UNION ALL
            SELECT created_at, CASE result WHEN 'ok' THEN 'used' ELSE result END AS kind, module_id AS source,
                   action || CASE WHEN target != '' THEN ': ' || target ELSE '' END AS detail, id * 2 + 1 AS ord
            FROM action_log WHERE permission = ?1
         ) ORDER BY created_at DESC, ord DESC LIMIT ?2",
    )
    .bind(id)
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?)
}

#[tauri::command]
pub async fn permissions_center(state: State<'_, AppState>) -> AppResult<Vec<PermissionInfo>> {
    center(&state.db).await
}

#[tauri::command]
pub async fn permissions_access_log(state: State<'_, AppState>, id: String, limit: Option<i64>) -> AppResult<Vec<AccessEntry>> {
    access_log(&state.db, &id, limit.unwrap_or(100)).await
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
