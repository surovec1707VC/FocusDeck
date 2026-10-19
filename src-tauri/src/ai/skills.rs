//! AI skills: JSON manifests in `skills/` loaded at startup. Skills only
//! produce plans and request tools through the AI Router; they never perform
//! actions on their own.

pub mod executor;
pub mod loader;
pub mod validator;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::State;

use super::profiles;
use super::providers::ProviderKind;
use super::AiRouter;
use crate::modules::ModuleRegistry;
use executor::SkillRunOutcome;
use loader::{LoadIssue, SkillManifest};

#[derive(Debug, Clone, Serialize)]
pub struct SkillInfo {
    pub manifest: SkillManifest,
    /// Current state (the manifest's `enabled` is only the default).
    pub enabled: bool,
    pub warnings: Vec<String>,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SkillLogEntry {
    pub step: i64,
    pub level: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillRunLog {
    pub task_id: String,
    pub status: String,
    pub created_at: String,
    pub entries: Vec<SkillLogEntry>,
}

struct Loaded {
    manifest: SkillManifest,
    warnings: Vec<String>,
    source_file: String,
}

#[derive(Default)]
struct LoadState {
    skills: Vec<Loaded>,
    issues: Vec<LoadIssue>,
}

pub struct SkillRegistry {
    dir: PathBuf,
    pool: SqlitePool,
    state: RwLock<LoadState>,
}

impl SkillRegistry {
    pub fn new(dir: PathBuf, pool: SqlitePool) -> Self {
        Self { dir, pool, state: RwLock::new(LoadState::default()) }
    }

    /// (Re)reads manifests from disk, validates them and syncs the `skills` table.
    /// Invalid manifests are reported as issues and not loaded.
    pub async fn reload(&self) -> anyhow::Result<()> {
        let (loaded, mut issues) = loader::load_dir(&self.dir);
        let mut skills = Vec::new();
        for l in loaded {
            let file = l.path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let v = validator::validate_manifest(&l.manifest, &l.path);
            if !v.errors.is_empty() {
                issues.push(LoadIssue { file, errors: v.errors });
                continue;
            }
            let m = &l.manifest;
            sqlx::query(
                "INSERT INTO skills (id, name, category, enabled, safety_level, manifest_json)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    category = excluded.category,
                    safety_level = excluded.safety_level,
                    manifest_json = excluded.manifest_json,
                    updated_at = datetime('now')",
            )
            .bind(&m.id)
            .bind(&m.name)
            .bind(&m.category)
            .bind(m.enabled)
            .bind(m.safety_level.as_str())
            .bind(serde_json::to_string(m)?)
            .execute(&self.pool)
            .await?;
            skills.push(Loaded { manifest: l.manifest, warnings: v.warnings, source_file: file });
        }
        for issue in &issues {
            tauri_plugin_log::log::warn!("skill {} not loaded: {:?}", issue.file, issue.errors);
        }
        *self.state.write().unwrap() = LoadState { skills, issues };
        Ok(())
    }

    async fn enabled_map(&self) -> sqlx::Result<HashMap<String, bool>> {
        let rows: Vec<(String, bool)> = sqlx::query_as("SELECT id, enabled FROM skills")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn list(&self) -> sqlx::Result<Vec<SkillInfo>> {
        let enabled = self.enabled_map().await?;
        let state = self.state.read().unwrap();
        Ok(state
            .skills
            .iter()
            .map(|s| SkillInfo {
                manifest: s.manifest.clone(),
                enabled: enabled.get(&s.manifest.id).copied().unwrap_or(s.manifest.enabled),
                warnings: s.warnings.clone(),
                source_file: s.source_file.clone(),
            })
            .collect())
    }

    pub fn issues(&self) -> Vec<LoadIssue> {
        self.state.read().unwrap().issues.clone()
    }

    async fn get(&self, id: &str) -> anyhow::Result<SkillInfo> {
        self.list()
            .await?
            .into_iter()
            .find(|s| s.manifest.id == id)
            .ok_or_else(|| anyhow::anyhow!("skill `{id}` not found"))
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> anyhow::Result<SkillInfo> {
        self.get(id).await?;
        sqlx::query("UPDATE skills SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get(id).await
    }

    /// Runs the skill planner and records the run in `agent_tasks` / `agent_logs`.
    pub async fn run(
        &self,
        router: &AiRouter,
        id: &str,
        input: Value,
        provider_override: Option<ProviderKind>,
        model_override: Option<String>,
    ) -> anyhow::Result<SkillRunOutcome> {
        let skill = self.get(id).await?;
        if !skill.enabled {
            anyhow::bail!("skill `{id}` is disabled");
        }
        let manifest = skill.manifest;
        let profile = profiles::get(&manifest.model_profile)
            .ok_or_else(|| anyhow::anyhow!("unknown model profile `{}`", manifest.model_profile))?;
        let kind = provider_override.unwrap_or(profile.provider);
        let model = match model_override.filter(|m| !m.trim().is_empty()) {
            Some(m) => m,
            None if kind == profile.provider => profile.model.to_string(),
            None => anyhow::bail!("choose a model for {}", kind.as_str()),
        };

        let task_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO agent_tasks (id, skill_id, status, input_json) VALUES (?, ?, 'running', ?)")
            .bind(&task_id)
            .bind(id)
            .bind(input.to_string())
            .execute(&self.pool)
            .await?;
        self.log(&task_id, 0, "info", &format!("started on {} / {model}", kind.as_str())).await?;

        let outcome = async {
            let provider = router.provider(kind)?;
            Ok::<_, anyhow::Error>(
                executor::run(
                    provider.as_ref(),
                    model,
                    profile.max_tokens,
                    profile.effort.map(str::to_string),
                    &manifest,
                    &input,
                )
                .await?,
            )
        }
        .await;

        match &outcome {
            Ok(out) => {
                for (i, step) in out.plan.iter().enumerate() {
                    let status = serde_json::to_value(&step.status)?;
                    self.log(&task_id, i as i64 + 1, "info", &format!("proposed {} — {}", step.tool, status))
                        .await?;
                }
                for w in &out.warnings {
                    self.log(&task_id, 0, "warn", w).await?;
                }
                let status = if out.requires_confirmation { "awaiting_confirmation" } else { "completed" };
                sqlx::query(
                    "UPDATE agent_tasks SET status = ?, plan_json = ?, result_json = ?, updated_at = datetime('now') WHERE id = ?",
                )
                .bind(status)
                .bind(serde_json::to_string(&out.plan)?)
                .bind(out.result.to_string())
                .bind(&task_id)
                .execute(&self.pool)
                .await?;
            }
            Err(e) => {
                self.log(&task_id, 0, "error", &e.to_string()).await?;
                sqlx::query("UPDATE agent_tasks SET status = 'failed', updated_at = datetime('now') WHERE id = ?")
                    .bind(&task_id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        outcome
    }

    async fn log(&self, task_id: &str, step: i64, level: &str, message: &str) -> sqlx::Result<()> {
        sqlx::query("INSERT INTO agent_logs (id, agent_task_id, step, level, message) VALUES (?, ?, ?, ?, ?)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(task_id)
            .bind(step)
            .bind(level)
            .bind(message)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The 20 most recent runs of a skill with their log entries.
    pub async fn logs(&self, id: &str) -> sqlx::Result<Vec<SkillRunLog>> {
        let tasks: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, status, created_at FROM agent_tasks WHERE skill_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 20",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(tasks.len());
        for (task_id, status, created_at) in tasks {
            let entries = sqlx::query_as::<_, SkillLogEntry>(
                "SELECT step, level, message, created_at FROM agent_logs WHERE agent_task_id = ? ORDER BY created_at, rowid",
            )
            .bind(&task_id)
            .fetch_all(&self.pool)
            .await?;
            out.push(SkillRunLog { task_id, status, created_at, entries });
        }
        Ok(out)
    }
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Debug, Deserialize)]
pub struct TestRunRequest {
    pub id: String,
    pub input: Value,
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,
}

#[tauri::command]
pub async fn skills_list(skills: State<'_, SkillRegistry>) -> CmdResult<Vec<SkillInfo>> {
    skills.list().await.map_err(err)
}

#[tauri::command]
pub fn skills_issues(skills: State<'_, SkillRegistry>) -> Vec<LoadIssue> {
    skills.issues()
}

#[tauri::command]
pub async fn skills_reload(skills: State<'_, SkillRegistry>) -> CmdResult<Vec<SkillInfo>> {
    skills.reload().await.map_err(err)?;
    skills.list().await.map_err(err)
}

#[tauri::command]
pub async fn skills_set_enabled(
    skills: State<'_, SkillRegistry>,
    id: String,
    enabled: bool,
) -> CmdResult<SkillInfo> {
    skills.set_enabled(&id, enabled).await.map_err(err)
}

#[tauri::command]
pub async fn skills_test_run(
    skills: State<'_, SkillRegistry>,
    modules: State<'_, ModuleRegistry>,
    router: State<'_, AiRouter>,
    request: TestRunRequest,
) -> CmdResult<SkillRunOutcome> {
    if !modules.get("skills").await.map_err(err)?.enabled {
        return Err("the Skills module is disabled in Control Center".into());
    }
    skills
        .run(&router, &request.id, request.input, request.provider, request.model)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn skills_logs(skills: State<'_, SkillRegistry>, id: String) -> CmdResult<Vec<SkillRunLog>> {
    skills.logs(&id).await.map_err(err)
}
