//! Runs a skill as a *planner*: the model returns a result plus a plan of
//! tool calls. Nothing in the plan is executed here — each step is checked
//! against the skill's allowed tools and marked for confirmation, and the
//! caller decides what (if anything) runs.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::loader::{SafetyLevel, SkillManifest};
use super::validator::{check_schema, find_tool};
use crate::ai::providers::{AiError, AiProvider, ChatMessage, ChatRequest, Role};

#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    #[error("invalid input: {}", .0.join("; "))]
    InvalidInput(Vec<String>),
    #[error(transparent)]
    Ai(#[from] AiError),
    #[error("the model declined this request")]
    Refused,
    #[error("model reply is not valid JSON: {0}")]
    BadReply(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StepStatus {
    /// Allowed; `needs_confirmation` says whether the user must approve it.
    Proposed { needs_confirmation: bool },
    /// Rejected before reaching the user.
    Blocked { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedStep {
    pub tool: String,
    pub args: Value,
    pub reason: String,
    #[serde(flatten)]
    pub status: StepStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillRunOutcome {
    pub summary: String,
    pub result: Value,
    pub plan: Vec<PlannedStep>,
    pub requires_confirmation: bool,
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
struct RawReply {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    plan: Vec<RawStep>,
}

#[derive(Deserialize)]
struct RawStep {
    tool: String,
    #[serde(default)]
    args: Value,
    #[serde(default)]
    reason: String,
}

fn build_system_prompt(skill: &SkillManifest) -> String {
    let tools = if skill.allowed_tools.is_empty() {
        "none — answer from the input alone, with an empty plan".to_string()
    } else {
        skill.allowed_tools.join(", ")
    };
    format!(
        "{prompt}\n\n\
         ## How to respond\n\
         You cannot call tools yourself. If you need a tool, put it in `plan` and FocusDeck \
         will ask the user before running it. Allowed tools: {tools}. At most {max} plan steps.\n\
         Reply with one JSON object and nothing else:\n\
         {{\"summary\": \"one or two sentences for the user\", \
         \"result\": <object matching the output schema below>, \
         \"plan\": [{{\"tool\": \"<allowed tool>\", \"args\": {{}}, \"reason\": \"why\"}}]}}\n\
         Output schema for `result`:\n{schema}",
        prompt = skill.system_prompt.trim(),
        max = skill.max_steps,
        schema = serde_json::to_string_pretty(&skill.output_schema).unwrap_or_default(),
    )
}

/// Pulls the JSON object out of a reply that may be wrapped in prose or a code fence.
fn extract_json(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

pub async fn run(
    provider: &dyn AiProvider,
    model: String,
    max_tokens: u32,
    skill: &SkillManifest,
    input: &Value,
) -> Result<SkillRunOutcome, ExecError> {
    let input_errors = check_schema(&skill.input_schema, input);
    if !input_errors.is_empty() {
        return Err(ExecError::InvalidInput(input_errors));
    }

    let request = ChatRequest {
        model,
        system: Some(build_system_prompt(skill)),
        messages: vec![ChatMessage {
            role: Role::User,
            content: format!(
                "Input:\n```json\n{}\n```",
                serde_json::to_string_pretty(input).unwrap_or_default()
            ),
        }],
        max_tokens,
    };
    let reply = provider.chat(&request).await?;
    if reply.stop_reason.as_deref() == Some("refusal") {
        return Err(ExecError::Refused);
    }

    let json = extract_json(&reply.content).ok_or_else(|| ExecError::BadReply("no JSON object found".into()))?;
    let raw: RawReply = serde_json::from_str(json).map_err(|e| ExecError::BadReply(e.to_string()))?;

    let mut warnings: Vec<String> = check_schema(&skill.output_schema, &raw.result)
        .into_iter()
        .map(|e| format!("result does not match output schema: {e}"))
        .collect();
    if reply.stop_reason.as_deref() == Some("max_tokens") {
        warnings.push("reply hit the token limit and may be incomplete".into());
    }

    let mut steps = raw.plan;
    if steps.len() > skill.max_steps as usize {
        warnings.push(format!(
            "plan had {} steps; only the first {} were kept",
            steps.len(),
            skill.max_steps
        ));
        steps.truncate(skill.max_steps as usize);
    }

    let plan: Vec<PlannedStep> = steps
        .into_iter()
        .map(|s| {
            let status = if !skill.allowed_tools.contains(&s.tool) {
                StepStatus::Blocked { reason: format!("`{}` is not in this skill's allowed tools", s.tool) }
            } else {
                match find_tool(&s.tool) {
                    None => StepStatus::Blocked { reason: format!("unknown tool `{}`", s.tool) },
                    Some(spec) => StepStatus::Proposed {
                        needs_confirmation: spec.mutating || skill.safety_level == SafetyLevel::Dangerous,
                    },
                }
            };
            PlannedStep {
                tool: s.tool,
                args: if s.args.is_null() { json!({}) } else { s.args },
                reason: s.reason,
                status,
            }
        })
        .collect();

    let requires_confirmation = plan
        .iter()
        .any(|s| matches!(s.status, StepStatus::Proposed { needs_confirmation: true }));

    Ok(SkillRunOutcome {
        summary: raw.summary,
        result: raw.result,
        plan,
        requires_confirmation,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::providers::{ChatStream, ModelInfo, ProviderKind, StreamChunk};
    use async_trait::async_trait;
    use futures_util::stream;

    struct FakeProvider(&'static str);

    #[async_trait]
    impl AiProvider for FakeProvider {
        fn kind(&self) -> ProviderKind {
            ProviderKind::Ollama
        }
        async fn stream_chat(&self, _: &ChatRequest) -> Result<ChatStream, AiError> {
            Ok(Box::pin(stream::iter(vec![
                Ok(StreamChunk::Text(self.0.to_string())),
                Ok(StreamChunk::Stop(Some("end_turn".into()))),
            ])))
        }
        async fn list_models(&self) -> Result<Vec<ModelInfo>, AiError> {
            Ok(vec![])
        }
        async fn test_connection(&self) -> Result<(), AiError> {
            Ok(())
        }
    }

    fn skill() -> SkillManifest {
        serde_json::from_value(json!({
            "id": "demo", "name": "Demo", "description": "d", "category": "c", "enabled": true,
            "required_permissions": ["tasks:write"], "allowed_tools": ["tasks.list", "tasks.create"],
            "system_prompt": "Plan things.",
            "input_schema": { "type": "object", "required": ["goal"], "properties": { "goal": { "type": "string" } } },
            "output_schema": { "type": "object", "required": ["tasks"] },
            "safety_level": "confirm_before_action", "max_steps": 2, "model_profile": "task_planner"
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn builds_checked_plan_without_executing() {
        let reply = r#"Sure! ```json
        {"summary": "Two tasks", "result": {"tasks": []},
         "plan": [
           {"tool": "tasks.list", "args": {}, "reason": "check duplicates"},
           {"tool": "tasks.create", "args": {"title": "x"}, "reason": "add"},
           {"tool": "shell.run", "args": {}, "reason": "extra"}
         ]}
        ```"#;
        let out = run(&FakeProvider(reply), "m".into(), 100, &skill(), &json!({ "goal": "g" }))
            .await
            .unwrap();
        assert_eq!(out.summary, "Two tasks");
        assert_eq!(out.plan.len(), 2, "truncated to max_steps");
        assert!(matches!(out.plan[0].status, StepStatus::Proposed { needs_confirmation: false }));
        assert!(matches!(out.plan[1].status, StepStatus::Proposed { needs_confirmation: true }));
        assert!(out.requires_confirmation);
        assert!(out.warnings.iter().any(|w| w.contains("only the first 2")));
    }

    #[tokio::test]
    async fn blocks_tools_outside_allow_list() {
        let reply = r#"{"summary": "", "result": {"tasks": []}, "plan": [{"tool": "shell.run", "args": {}}]}"#;
        let out = run(&FakeProvider(reply), "m".into(), 100, &skill(), &json!({ "goal": "g" }))
            .await
            .unwrap();
        assert!(matches!(out.plan[0].status, StepStatus::Blocked { .. }));
        assert!(!out.requires_confirmation);
    }

    #[tokio::test]
    async fn rejects_invalid_input_and_bad_reply() {
        let err = run(&FakeProvider("{}"), "m".into(), 100, &skill(), &json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, ExecError::InvalidInput(_)));

        let err = run(&FakeProvider("no json here"), "m".into(), 100, &skill(), &json!({ "goal": "g" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ExecError::BadReply(_)));
    }
}
