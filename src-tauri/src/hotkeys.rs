//! Global hotkeys: the show/hide window shortcut and hotkey-triggered
//! automations. Nothing is registered unless the «Глобальные горячие
//! клавиши» module, the setting and the permission are all on. Automation
//! shortcuts also need the Automations module and the automation's own module;
//! layout shortcuts need the Layouts module; Clipboard Hub shortcuts (open
//! the hub, quick actions) need the Clipboard Hub module.

use std::str::FromStr;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::automation::{engine, store, types::Trigger};
use crate::error::{AppError, AppResult};
use crate::modules::catalog::{AUTOMATIONS, CLIPBOARD, HOTKEYS, LAYOUTS};
use crate::modules::registry;
use crate::permissions::{self, PERM_GLOBAL_HOTKEYS};
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyTarget {
    ToggleWindow,
    Automation(String),
    Layout(String),
    /// Show the window on the Clipboard Hub page.
    ClipboardHub,
    QuickAction(String),
}

/// Called by the global-shortcut plugin for every registered shortcut.
pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let target = app.state::<AppState>().hotkeys.lock().unwrap().get(&shortcut.id()).cloned();
    match target {
        Some(HotkeyTarget::ToggleWindow) => crate::window::toggle(app),
        Some(HotkeyTarget::Automation(id)) => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = engine::run_by_id(&app, &id).await {
                    tracing::warn!("hotkey automation {id}: {e}");
                }
            });
        }
        Some(HotkeyTarget::ClipboardHub) => {
            crate::window::show(app);
            let _ = tauri::Emitter::emit(app, crate::tray::NAVIGATE_EVENT, "/clipboard");
        }
        Some(HotkeyTarget::QuickAction(id)) => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::clipboard_hub::actions::run(&app, &id, false).await {
                    tracing::warn!("hotkey quick action {id}: {e}");
                    let _ = crate::notifications::push(&app, "error", "Быстрое действие не выполнено", &e.to_string(), "Clipboard Hub").await;
                }
            });
        }
        Some(HotkeyTarget::Layout(id)) => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::workspace::activate_from_hotkey(&app, &id).await {
                    tracing::warn!("hotkey layout {id}: {e}");
                }
            });
        }
        None => {}
    }
}

/// Global hotkeys are allowed: setting, module and permission are all on.
pub async fn active(app: &AppHandle) -> AppResult<bool> {
    let state = app.state::<AppState>();
    Ok(state.settings().global_hotkeys_enabled
        && registry::is_enabled(&state.db, HOTKEYS).await?
        && permissions::is_granted(&state.db, PERM_GLOBAL_HOTKEYS).await?)
}

/// Whether the layout's shortcut is registered right now.
pub fn layout_registered(app: &AppHandle, layout_id: &str) -> bool {
    let target = HotkeyTarget::Layout(layout_id.to_string());
    app.state::<AppState>().hotkeys.lock().unwrap().values().any(|t| *t == target)
}

/// Re-registers all shortcuts from current modules, settings and automations.
/// Fails only if the window shortcut can't be registered; automation
/// shortcuts that are taken are logged and skipped.
pub async fn sync(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(anyhow::Error::from)?;
    state.hotkeys.lock().unwrap().clear();

    let settings = state.settings();
    if !active(app).await? {
        tracing::info!("global hotkeys are off");
        return Ok(());
    }

    let register = |text: &str, target: HotkeyTarget| -> AppResult<()> {
        let shortcut = Shortcut::from_str(text.trim())
            .map_err(|e| AppError::Invalid(format!("сочетание «{text}» не распознано: {e}")))?;
        let mut map = state.hotkeys.lock().unwrap();
        if map.contains_key(&shortcut.id()) {
            return Err(AppError::Invalid(format!("сочетание «{text}» уже используется в FocusDeck")));
        }
        gs.register(shortcut)
            .map_err(|_| AppError::Invalid(format!("сочетание «{text}» занято другой программой")))?;
        map.insert(shortcut.id(), target);
        Ok(())
    };

    if !settings.toggle_window_hotkey.trim().is_empty() {
        register(&settings.toggle_window_hotkey, HotkeyTarget::ToggleWindow)?;
    }

    if registry::is_enabled(&state.db, AUTOMATIONS).await? {
        for a in store::list(&state.db).await?.into_iter().filter(|a| a.enabled) {
            let Trigger::Hotkey { shortcut } = &a.trigger else { continue };
            if !registry::is_enabled(&state.db, &a.module_id).await? {
                continue;
            }
            if let Err(e) = register(shortcut, HotkeyTarget::Automation(a.id.clone())) {
                tracing::warn!(automation = %a.id, "{e}");
                let _ = store::log(&state.db, &a.id, "error", &e.to_string(), 500).await;
            }
        }
    }
    if registry::is_enabled(&state.db, LAYOUTS).await? {
        for l in crate::workspace::layouts(&state.db).await? {
            let Some(shortcut) = &l.hotkey else { continue };
            if let Err(e) = register(shortcut, HotkeyTarget::Layout(l.id.clone())) {
                tracing::warn!(layout = %l.name, "{e}");
            }
        }
    }
    if registry::is_enabled(&state.db, CLIPBOARD).await? {
        let open = registry::settings(&state.db, CLIPBOARD).await?;
        if let Some(h) = open.get("open_hotkey").and_then(serde_json::Value::as_str).filter(|h| !h.trim().is_empty()) {
            if let Err(e) = register(h, HotkeyTarget::ClipboardHub) {
                tracing::warn!("clipboard hub hotkey: {e}");
            }
        }
        for a in crate::clipboard_hub::actions::list(&state.db).await?.into_iter().filter(|a| a.kind != "command") {
            let Some(shortcut) = &a.hotkey else { continue };
            if let Err(e) = register(shortcut, HotkeyTarget::QuickAction(a.id.clone())) {
                tracing::warn!(action = %a.name, "{e}");
            }
        }
    }
    tracing::info!(count = state.hotkeys.lock().unwrap().len(), "global hotkeys registered");
    Ok(())
}

/// Re-registers everything and fails if `expected` could not be registered
/// (taken by another program or already used in FocusDeck) while hotkeys are on.
pub async fn sync_reporting(app: &AppHandle, expected: Option<HotkeyTarget>) -> AppResult<()> {
    if let Err(e) = sync(app).await {
        tracing::warn!("{e}");
    }
    let Some(target) = expected else { return Ok(()) };
    if active(app).await? && !app.state::<AppState>().hotkeys.lock().unwrap().values().any(|t| *t == target) {
        return Err(AppError::Invalid("сочетание не удалось назначить: оно занято другой программой или уже используется в FocusDeck".into()));
    }
    Ok(())
}
