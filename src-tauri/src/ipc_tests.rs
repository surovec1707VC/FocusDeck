//! Integration tests through the real IPC layer: a mock Tauri app with the
//! production `AppState`, commands invoked by name with JSON arguments, the
//! way the frontend calls them. Checks argument names (camelCase → snake_case),
//! serialization of results and the `{ kind, message }` error shape.
//!
//! Commands that need an `AppHandle<Wry>` (dialogs, windows, OS integration)
//! are covered by their module's unit tests and the UI smoke test instead.

use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

use crate::state::AppState;

async fn app() -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let pool = crate::db::test_pool().await;
    crate::permissions::seed(&pool).await.unwrap();
    crate::modules::registry::seed(&pool).await.unwrap();
    crate::workspace::seed(&pool).await.unwrap();
    let settings = crate::settings::load(&pool).await.unwrap();
    let enabled = crate::modules::registry::enabled_set(&pool).await.unwrap();
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![
            crate::modules::list_modules,
            crate::modules::get_module,
            crate::modules::get_module_logs,
            crate::permissions::permissions_center,
            crate::permissions::permissions_list,
            crate::productivity::notes::notes_create,
            crate::productivity::notes::notes_list,
            crate::productivity::notes::notes_update,
            crate::productivity::notes::notes_trash,
            crate::productivity::tasks::tasks_create,
            crate::productivity::tasks::tasks_query,
            crate::automation::automations_list,
            crate::automation::automations_logs,
            crate::clipboard_hub::clipboard_list,
            crate::clipboard_hub::clipboard_pin,
            crate::clipboard_hub::clipboard_delete,
            crate::organizer::organizer_rules,
            crate::organizer::organizer_save_rule,
            crate::organizer::organizer_delete_rule,
            crate::session::session_status,
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(AppState::new(pool, settings, enabled));
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    (app, webview)
}

/// Invokes a command; Ok(result) or Err(`{ kind, message }`).
fn call(w: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    let req = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(w, req).map(|b| b.deserialize::<Value>().unwrap())
}

fn set_module(app: &tauri::App<MockRuntime>, id: &str, on: bool) {
    let state = app.state::<AppState>();
    let mut set = state.enabled_modules.write().unwrap();
    if on {
        set.insert(id.into());
    } else {
        set.remove(id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn modules_and_permissions() {
    let (_app, w) = app().await;
    let modules = call(&w, "list_modules", json!({})).unwrap();
    assert!(modules.as_array().unwrap().len() >= 40);
    let m = call(&w, "get_module", json!({ "id": "notes" })).unwrap();
    assert_eq!(m["id"], "notes");
    let err = call(&w, "get_module", json!({ "id": "nope" })).unwrap_err();
    assert_eq!(err["kind"], "not_found");
    assert!(err["message"].as_str().unwrap().len() > 3);
    // Missing argument is a deserialization error, not a panic.
    assert!(call(&w, "get_module", json!({})).is_err());
    let perms = call(&w, "permissions_center", json!({})).unwrap();
    // Every risky permission starts revoked.
    for id in ["system.run_commands", "files.delete", "network.api_tester", "network.updates", "clipboard.read"] {
        let p = perms.as_array().unwrap().iter().find(|p| p["id"] == id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(p["granted"], false, "{id} must be off by default");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn notes_and_tasks_crud() {
    let (app, w) = app().await;
    let note = call(&w, "notes_create", json!({ "input": { "title": "IPC", "body": "**text**", "tags": ["#Work", "work"] } })).unwrap();
    assert_eq!(note["title"], "IPC");
    assert_eq!(note["tags"], json!(["work"]));
    let id = note["id"].as_str().unwrap();
    let upd = call(&w, "notes_update", json!({ "id": id, "input": { "title": "IPC 2" } })).unwrap();
    assert_eq!(upd["title"], "IPC 2");
    assert!(call(&w, "notes_list", json!({})).unwrap().as_array().unwrap().iter().any(|n| n["id"] == id));
    call(&w, "notes_trash", json!({ "id": id })).unwrap();
    // Validation errors reach the UI with kind "invalid".
    let err = call(&w, "notes_create", json!({ "input": { "title": "x".repeat(10_000) } })).unwrap_err();
    assert_eq!(err["kind"], "invalid");

    let task = call(&w, "tasks_create", json!({ "input": { "title": "Задача", "priority": 3 } })).unwrap();
    assert_eq!(task["priority"], 3);
    assert_eq!(call(&w, "tasks_create", json!({ "input": { "title": "", } })).unwrap_err()["kind"], "invalid");
    assert_eq!(call(&w, "tasks_create", json!({ "input": { "title": "a", "priority": 9 } })).unwrap_err()["kind"], "invalid");

    // A turned-off module answers module_disabled.
    set_module(&app, "notes", false);
    assert_eq!(call(&w, "notes_list", json!({})).unwrap_err()["kind"], "module_disabled");
}

#[tokio::test(flavor = "multi_thread")]
async fn clipboard_history() {
    let (app, w) = app().await;
    set_module(&app, "clipboard_history", true);
    let db = app.state::<AppState>().db.clone();
    let clip = |t: &str| crate::clipboard_hub::win::Clip { text: Some(t.into()), ..Default::default() };
    for t in ["one", "two", "three"] {
        crate::clipboard_hub::store::add(&db, &clip(t), "test.exe", 2).await.unwrap();
    }
    let page = call(&w, "clipboard_list", json!({})).unwrap();
    // max_items = 2: the oldest unpinned entry is gone.
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["content"], "three");
    let id = items[1]["id"].as_i64().unwrap();
    call(&w, "clipboard_pin", json!({ "id": id, "pinned": true })).unwrap();
    // Pinned entries survive trimming; a repeated copy doesn't duplicate.
    for t in ["four", "five", "two"] {
        crate::clipboard_hub::store::add(&db, &clip(t), "test.exe", 2).await.unwrap();
    }
    let contents: Vec<String> = call(&w, "clipboard_list", json!({})).unwrap()["items"].as_array().unwrap().iter().map(|i| i["content"].as_str().unwrap().to_string()).collect();
    assert_eq!(contents.iter().filter(|c| *c == "two").count(), 1);
    call(&w, "clipboard_delete", json!({ "id": id })).unwrap();
    assert_eq!(call(&w, "clipboard_delete", json!({ "id": -1 })).map(|_| ()).unwrap_or(()), ());
}

#[tokio::test(flavor = "multi_thread")]
async fn organizer_rules() {
    let (app, w) = app().await;
    set_module(&app, "desktop_organizer", true);
    let rules = call(&w, "organizer_rules", json!({})).unwrap();
    let before = rules.as_array().unwrap().len();
    let rule = json!({ "name": "PDF", "enabled": true, "match": { "extensions": ["pdf"] }, "target": "%USERPROFILE%\\Documents\\PDF" });
    let after = call(&w, "organizer_save_rule", json!({ "input": rule })).unwrap();
    assert_eq!(after.as_array().unwrap().len(), before + 1);
    // A rule must match something and have a target.
    let bad = json!({ "name": "x", "enabled": true, "match": {}, "target": "" });
    assert_eq!(call(&w, "organizer_save_rule", json!({ "input": bad })).unwrap_err()["kind"], "invalid");
    let id = after.as_array().unwrap().iter().find(|r| r["name"] == "PDF").unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(call(&w, "organizer_delete_rule", json!({ "id": id })).unwrap().as_array().unwrap().len(), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn automations_and_session() {
    let (app, w) = app().await;
    set_module(&app, "automations", true);
    assert!(call(&w, "automations_list", json!({})).unwrap().is_array());
    assert!(call(&w, "automations_logs", json!({ "limit": 5 })).unwrap().is_array());
    let s = call(&w, "session_status", json!({})).unwrap();
    assert!(s.get("previous_crash").is_some());
}
