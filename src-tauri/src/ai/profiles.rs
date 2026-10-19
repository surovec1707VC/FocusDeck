//! AI Profiles: named presets of provider, model, system prompt and effort.

use serde::Serialize;

use super::providers::{anthropic, ProviderKind};

#[derive(Debug, Clone, Serialize)]
pub struct AiProfile {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub provider: ProviderKind,
    pub model: &'static str,
    pub system_prompt: &'static str,
    pub max_tokens: u32,
    /// Anthropic effort level; other providers ignore it.
    pub effort: Option<&'static str>,
}

// Streaming requests get a generous max_tokens so long answers aren't cut off.
const MAX_TOKENS: u32 = 64_000;

pub fn all() -> Vec<AiProfile> {
    vec![
        AiProfile {
            id: "fast_assistant",
            name: "Fast Assistant",
            description: "Short, direct answers for everyday questions.",
            provider: ProviderKind::Anthropic,
            model: anthropic::DEFAULT_MODEL,
            system_prompt: "You are FocusDeck's assistant. Answer briefly and directly. \
                            Use lists only when they make the answer easier to scan.",
            max_tokens: MAX_TOKENS,
            effort: Some("low"),
        },
        AiProfile {
            id: "coding_assistant",
            name: "Coding Assistant",
            description: "Writes and explains code; careful about correctness.",
            provider: ProviderKind::Anthropic,
            model: anthropic::DEFAULT_MODEL,
            system_prompt: "You are a senior software engineer. Give working code that matches \
                            the user's language and style, explain non-obvious decisions, and \
                            point out risks or edge cases you notice.",
            max_tokens: MAX_TOKENS,
            effort: Some("high"),
        },
        AiProfile {
            id: "prompt_improver",
            name: "Prompt Improver",
            description: "Rewrites a prompt to be clearer and more specific.",
            provider: ProviderKind::Anthropic,
            model: anthropic::DEFAULT_MODEL,
            system_prompt: "Rewrite the user's prompt so another AI model can act on it: state \
                            the goal, context, constraints and the expected output format. \
                            Return the improved prompt, then a short list of what you changed.",
            max_tokens: MAX_TOKENS,
            effort: Some("medium"),
        },
        AiProfile {
            id: "error_explainer",
            name: "Error Explainer",
            description: "Explains an error message or stack trace and how to fix it.",
            provider: ProviderKind::Anthropic,
            model: anthropic::DEFAULT_MODEL,
            system_prompt: "The user pastes an error, log or stack trace. Explain the most likely \
                            cause in plain language, then give concrete fix steps. If the input \
                            is not enough to be sure, say what extra information would help.",
            max_tokens: MAX_TOKENS,
            effort: Some("medium"),
        },
        AiProfile {
            id: "task_planner",
            name: "Task Planner",
            description: "Breaks a goal into ordered, actionable steps.",
            provider: ProviderKind::Anthropic,
            model: anthropic::DEFAULT_MODEL,
            system_prompt: "Turn the user's goal into a plan of small, ordered, verifiable steps. \
                            Note dependencies and open questions. Do not perform any actions \
                            yourself; only produce the plan.",
            max_tokens: MAX_TOKENS,
            effort: Some("high"),
        },
        AiProfile {
            id: "local_private_assistant",
            name: "Local Private Assistant",
            description: "Runs entirely on this machine through Ollama; nothing leaves the PC.",
            provider: ProviderKind::Ollama,
            model: "llama3.2",
            system_prompt: "You are a helpful assistant running locally. Answer concisely.",
            max_tokens: 4_096,
            effort: None,
        },
    ]
}

pub fn get(id: &str) -> Option<AiProfile> {
    all().into_iter().find(|p| p.id == id)
}
