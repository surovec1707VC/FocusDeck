//! Row types for the tables used from Rust today. Other tables get models
//! as their features land.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SettingRow {
    pub key: String,
    pub value_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ModuleRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    pub settings_json: String,
    pub requires_restart: bool,
    pub permissions_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiSessionRow {
    pub id: String,
    pub title: String,
    pub profile_id: String,
    pub provider: String,
    pub model: String,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiMessageRow {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConnectorRow {
    pub id: String,
    pub name: String,
    pub category: String,
    pub auth_type: String,
    pub enabled: bool,
    pub settings_json: String,
    pub last_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub category: String,
    pub enabled: bool,
    pub safety_level: String,
    pub manifest_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ClipboardEntryRow {
    pub id: String,
    pub content: String,
    pub kind: String,
    pub pinned: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NoteRow {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}
