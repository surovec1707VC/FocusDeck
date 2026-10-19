//! Turns a transcript into an intent. A fast local keyword matcher handles
//! common commands (Russian and English); anything else goes to the AI
//! (`voice-intent-router` skill) or, when `auto_send_to_ai` is on, straight
//! to the assistant. Nothing here executes the intent.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "intent", rename_all = "snake_case")]
pub enum Intent {
    Open { target: String },
    CreateNote { text: String },
    CreateTask { text: String },
    AskAi { text: String },
    /// Nothing matched locally; ask the voice-intent-router skill.
    Unknown { text: String },
}

const OPEN: &[&str] = &["открой", "открыть", "покажи", "перейди в", "open", "show", "go to"];
const NOTE: &[&str] = &["запиши заметку", "создай заметку", "заметка", "note that", "take a note", "new note"];
const TASK: &[&str] = &["добавь задачу", "создай задачу", "напомни", "задача", "add task", "new task", "remind me to"];
const ASK: &[&str] = &["спроси", "вопрос", "ask", "question"];

/// Strips the first matching prefix (case-insensitive) and returns the rest.
fn strip_prefix<'a>(text: &'a str, lower: &str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|p| {
        if lower.starts_with(p) {
            // Prefixes are ASCII or Cyrillic; lowercase keeps byte length for both.
            let rest = text.get(p.len()..)?;
            Some(rest.trim_start_matches([' ', ',', ':', '-']).trim())
        } else {
            None
        }
    })
}

pub fn route_local(transcript: &str, auto_send_to_ai: bool) -> Intent {
    let text = transcript.trim();
    let lower = text.to_lowercase();

    if let Some(rest) = strip_prefix(text, &lower, NOTE) {
        return Intent::CreateNote { text: rest.to_string() };
    }
    if let Some(rest) = strip_prefix(text, &lower, TASK) {
        return Intent::CreateTask { text: rest.to_string() };
    }
    if let Some(rest) = strip_prefix(text, &lower, OPEN) {
        return Intent::Open { target: rest.trim_end_matches(['.', '!']).to_lowercase() };
    }
    if let Some(rest) = strip_prefix(text, &lower, ASK) {
        return Intent::AskAi { text: rest.to_string() };
    }
    if auto_send_to_ai {
        Intent::AskAi { text: text.to_string() }
    } else {
        Intent::Unknown { text: text.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_common_commands() {
        assert_eq!(
            route_local("Запиши заметку: купить молоко", false),
            Intent::CreateNote { text: "купить молоко".into() }
        );
        assert_eq!(
            route_local("добавь задачу написать тесты", false),
            Intent::CreateTask { text: "написать тесты".into() }
        );
        assert_eq!(route_local("Открой настройки.", false), Intent::Open { target: "настройки".into() });
        assert_eq!(route_local("open skills", false), Intent::Open { target: "skills".into() });
        assert_eq!(
            route_local("Ask what is Rust", false),
            Intent::AskAi { text: "what is Rust".into() }
        );
    }

    #[test]
    fn unmatched_depends_on_auto_send() {
        assert_eq!(route_local("какая погода", false), Intent::Unknown { text: "какая погода".into() });
        assert_eq!(route_local("какая погода", true), Intent::AskAi { text: "какая погода".into() });
    }
}
