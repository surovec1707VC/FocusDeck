//! AI layer: providers, router, profiles, sessions and Tauri commands.

pub mod profiles;
pub mod providers;
pub mod router;
pub mod session;
pub mod skills;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::ipc::Channel;
use tauri::State;

use crate::db::models::{AiMessageRow, AiSessionRow};
use profiles::AiProfile;
use providers::{ChatRequest, ModelInfo, ProviderKind, Role, StreamChunk};
pub use router::AiRouter;
use router::RouterConfig;

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Events pushed to the frontend while a reply streams.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Started { session_id: String },
    Delta { text: String },
    Done { stop_reason: Option<String> },
    Error { message: String },
}

#[derive(Debug, Deserialize)]
pub struct ChatStreamRequest {
    pub session_id: Option<String>,
    pub profile_id: String,
    /// Overrides the profile's provider/model when set.
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,
    pub message: String,
}

#[tauri::command]
pub fn ai_list_profiles() -> Vec<AiProfile> {
    profiles::all()
}

#[tauri::command]
pub fn ai_get_config(router: State<'_, AiRouter>) -> RouterConfig {
    router.config()
}

#[tauri::command]
pub async fn ai_set_config(
    router: State<'_, AiRouter>,
    pool: State<'_, SqlitePool>,
    config: RouterConfig,
) -> CmdResult<()> {
    router.set_config(&pool, config).await.map_err(err)
}

// `endpoint_id` selects an OpenAI-compatible endpoint; `None` means the active one.

#[tauri::command]
pub async fn ai_list_models(
    router: State<'_, AiRouter>,
    provider: ProviderKind,
    endpoint_id: Option<String>,
) -> CmdResult<Vec<ModelInfo>> {
    let provider = router.provider_for(provider, endpoint_id.as_deref()).map_err(err)?;
    provider.list_models().await.map_err(err)
}

#[tauri::command]
pub async fn ai_test_connection(
    router: State<'_, AiRouter>,
    provider: ProviderKind,
    endpoint_id: Option<String>,
) -> CmdResult<()> {
    let provider = router.provider_for(provider, endpoint_id.as_deref()).map_err(err)?;
    provider.test_connection().await.map_err(err)
}

/// Stores an API key in the OS keyring. The key never comes back to the frontend.
#[tauri::command]
pub fn ai_set_api_key(
    router: State<'_, AiRouter>,
    provider: ProviderKind,
    endpoint_id: Option<String>,
    api_key: String,
) -> CmdResult<()> {
    let key = router
        .secret_key(provider, endpoint_id.as_deref())
        .map_err(err)?
        .ok_or("this provider does not use an API key")?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("API key is empty".into());
    }
    router.secrets().set(&key, api_key).map_err(err)
}

#[tauri::command]
pub fn ai_has_api_key(router: State<'_, AiRouter>, provider: ProviderKind, endpoint_id: Option<String>) -> CmdResult<bool> {
    match router.secret_key(provider, endpoint_id.as_deref()).map_err(err)? {
        Some(key) => router.secrets().has(&key).map_err(err),
        None => Ok(true),
    }
}

#[tauri::command]
pub fn ai_delete_api_key(router: State<'_, AiRouter>, provider: ProviderKind, endpoint_id: Option<String>) -> CmdResult<()> {
    match router.secret_key(provider, endpoint_id.as_deref()).map_err(err)? {
        Some(key) => router.secrets().delete(&key).map_err(err),
        None => Ok(()),
    }
}

#[derive(Debug, Serialize)]
pub struct ResolvedTarget {
    pub provider: ProviderKind,
    pub model: String,
    /// Name of the OpenAI-compatible endpoint in use, when that provider is chosen.
    pub endpoint_name: Option<String>,
}

/// Provider and model a profile uses right now (default provider applied).
#[tauri::command]
pub fn ai_resolve_target(router: State<'_, AiRouter>, profile_id: String) -> CmdResult<ResolvedTarget> {
    let profile = profiles::get(&profile_id).ok_or("unknown profile")?;
    let config = router.config();
    // A missing default model is not an error here: the UI lets the user pick one.
    let (provider, model) = router
        .resolve(&profile, None, None)
        .unwrap_or_else(|_| (config.default_provider.filter(|_| !profile.pinned_provider).unwrap_or(profile.provider), String::new()));
    let endpoint_name = (provider == ProviderKind::OpenaiCompatible)
        .then(|| config.endpoint(None).ok().map(|e| e.name.clone()))
        .flatten();
    Ok(ResolvedTarget { provider, model, endpoint_name })
}

#[tauri::command]
pub async fn ai_list_sessions(pool: State<'_, SqlitePool>) -> CmdResult<Vec<AiSessionRow>> {
    session::list(&pool).await.map_err(err)
}

#[tauri::command]
pub async fn ai_get_messages(pool: State<'_, SqlitePool>, session_id: String) -> CmdResult<Vec<AiMessageRow>> {
    session::messages(&pool, &session_id).await.map_err(err)
}

#[tauri::command]
pub async fn ai_delete_session(pool: State<'_, SqlitePool>, session_id: String) -> CmdResult<()> {
    session::delete(&pool, &session_id).await.map_err(err)
}

/// Sends a user message and streams the reply over `on_event`.
/// Returns the session id (a new one when `session_id` was not given).
#[tauri::command]
pub async fn ai_chat_stream(
    router: State<'_, AiRouter>,
    pool: State<'_, SqlitePool>,
    request: ChatStreamRequest,
    on_event: Channel<StreamEvent>,
) -> CmdResult<String> {
    let profile = profiles::get(&request.profile_id).ok_or("unknown profile")?;
    let (kind, model) = router.resolve(&profile, request.provider, request.model).map_err(err)?;
    let message = request.message.trim().to_string();
    if message.is_empty() {
        return Err("message is empty".into());
    }

    let session_id = match request.session_id {
        Some(id) => id,
        None => {
            let title: String = message.chars().take(60).collect();
            session::create(&pool, &title, profile.id, kind.as_str(), &model)
                .await
                .map_err(err)?
        }
    };
    let _ = on_event.send(StreamEvent::Started { session_id: session_id.clone() });

    session::add_message(&pool, &session_id, Role::User, &message)
        .await
        .map_err(err)?;
    let history = session::messages(&pool, &session_id).await.map_err(err)?;

    let chat = ChatRequest {
        model,
        system: Some(profile.system_prompt.to_string()),
        messages: session::to_chat_messages(&history),
        max_tokens: profile.max_tokens,
    };

    let mut reply = String::new();
    let mut stop_reason = None;
    let result: Result<(), String> = async {
        let provider = router.provider(kind).map_err(err)?;
        let mut stream = provider.stream_chat(&chat).await.map_err(err)?;
        while let Some(chunk) = stream.next().await {
            match chunk.map_err(err)? {
                StreamChunk::Text(text) => {
                    reply.push_str(&text);
                    let _ = on_event.send(StreamEvent::Delta { text });
                }
                StreamChunk::Stop(reason) => stop_reason = reason,
            }
        }
        Ok(())
    }
    .await;

    // Keep whatever arrived, even if the stream broke midway.
    if !reply.is_empty() {
        session::add_message(&pool, &session_id, Role::Assistant, &reply)
            .await
            .map_err(err)?;
    }
    let _ = match result {
        Ok(()) => on_event.send(StreamEvent::Done { stop_reason }),
        Err(message) => on_event.send(StreamEvent::Error { message }),
    };
    Ok(session_id)
}
