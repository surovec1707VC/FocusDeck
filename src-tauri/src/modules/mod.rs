//! Module Registry: every FocusDeck feature is a module that can be enabled,
//! disabled and configured from the Control Center.

pub mod registry;
pub mod settings;
pub mod status;
pub mod types;

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::State;

pub use registry::ModuleRegistry;
use status::{ModuleLogEntry, ModuleStatus};
use types::{Module, ModuleCategory, ModuleInfo, ModuleManifest};

/// A module with no runtime behaviour yet: it only carries a manifest so it
/// shows up in the Control Center. Real modules replace these one by one.
struct PlaceholderModule(ModuleManifest);

impl Module for PlaceholderModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.0
    }
}

fn placeholder(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    category: ModuleCategory,
    enabled_by_default: bool,
    permissions: Vec<&'static str>,
    settings_schema: Value,
) -> Arc<dyn Module> {
    Arc::new(PlaceholderModule(ModuleManifest {
        id,
        name,
        description,
        category,
        enabled_by_default,
        requires_restart: false,
        permissions,
        settings_schema,
    }))
}

pub fn builtin_modules() -> Vec<Arc<dyn Module>> {
    vec![
        placeholder(
            "ai_router",
            "AI Router",
            "Routes chat requests to Ollama, OpenAI-compatible or Anthropic providers.",
            ModuleCategory::Ai,
            true,
            vec!["http:ai-providers", "secrets:read"],
            json!({ "properties": {
                "default_profile": { "type": "string", "title": "Default profile", "default": "fast_assistant" },
                "ollama_url": { "type": "string", "title": "Ollama URL", "default": "http://localhost:11434" },
                "stream": { "type": "boolean", "title": "Stream responses", "default": true }
            }}),
        ),
        placeholder(
            "skills",
            "Skills",
            "AI skills loaded from skills/*.json manifests.",
            ModuleCategory::Ai,
            true,
            vec!["fs:read-app"],
            json!({ "properties": {
                "require_confirmation": { "type": "boolean", "title": "Confirm before actions", "default": true }
            }}),
        ),
        placeholder(
            "connectors",
            "Connectors",
            "Integrations with external tools and services.",
            ModuleCategory::Integrations,
            true,
            vec!["http:connectors", "secrets:read"],
            json!({ "properties": {} }),
        ),
        placeholder(
            "clipboard_history",
            "Clipboard History",
            "Keeps a searchable history of copied text.",
            ModuleCategory::Productivity,
            false,
            vec!["clipboard:read"],
            json!({ "properties": {
                "max_items": { "type": "integer", "title": "Max items", "default": 200 },
                "ignore_passwords": { "type": "boolean", "title": "Skip password-like entries", "default": true }
            }}),
        ),
        placeholder(
            "snippets",
            "Snippets",
            "Reusable text snippets with quick insert.",
            ModuleCategory::Productivity,
            false,
            vec!["clipboard:write"],
            json!({ "properties": {} }),
        ),
        placeholder(
            "notes",
            "Notes",
            "Quick notes linked to projects.",
            ModuleCategory::Productivity,
            true,
            vec![],
            json!({ "properties": {} }),
        ),
        placeholder(
            "automations",
            "Automations",
            "Trigger → action rules. Every action needs permission.",
            ModuleCategory::Automation,
            false,
            vec!["shell:execute", "input:simulate"],
            json!({ "properties": {
                "dry_run": { "type": "boolean", "title": "Dry run (log only)", "default": true }
            }}),
        ),
        placeholder(
            "voice",
            "Voice Control",
            "Speech-to-text, text-to-speech and voice intents. Configured on the Voice page.",
            ModuleCategory::Voice,
            false,
            vec!["microphone", "global-shortcut"],
            json!({ "properties": {} }),
        ),
        placeholder(
            "system_monitor",
            "System Monitor",
            "CPU, memory and process stats for widgets.",
            ModuleCategory::System,
            false,
            vec!["system:info"],
            json!({ "properties": {
                "interval_secs": { "type": "integer", "title": "Refresh interval (s)", "default": 5 }
            }}),
        ),
    ]
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn list_modules(registry: State<'_, ModuleRegistry>) -> CmdResult<Vec<ModuleInfo>> {
    registry.list().await.map_err(err)
}

#[tauri::command]
pub async fn get_module(registry: State<'_, ModuleRegistry>, id: String) -> CmdResult<ModuleInfo> {
    registry.get(&id).await.map_err(err)
}

#[tauri::command]
pub async fn enable_module(registry: State<'_, ModuleRegistry>, id: String) -> CmdResult<ModuleInfo> {
    registry.enable(&id).await.map_err(err)
}

#[tauri::command]
pub async fn disable_module(registry: State<'_, ModuleRegistry>, id: String) -> CmdResult<ModuleInfo> {
    registry.disable(&id).await.map_err(err)
}

#[tauri::command]
pub async fn update_module_settings(
    registry: State<'_, ModuleRegistry>,
    id: String,
    settings: Value,
) -> CmdResult<ModuleInfo> {
    registry.update_settings(&id, settings).await.map_err(err)
}

#[tauri::command]
pub async fn reset_module_settings(registry: State<'_, ModuleRegistry>, id: String) -> CmdResult<ModuleInfo> {
    registry.reset_settings(&id).await.map_err(err)
}

#[tauri::command]
pub fn get_module_logs(registry: State<'_, ModuleRegistry>, id: String) -> CmdResult<Vec<ModuleLogEntry>> {
    registry.logs(&id).map_err(err)
}

#[tauri::command]
pub fn get_module_status(registry: State<'_, ModuleRegistry>, id: String) -> CmdResult<ModuleStatus> {
    registry.status(&id).map_err(err)
}
