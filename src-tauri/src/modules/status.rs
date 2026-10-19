use std::collections::VecDeque;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum ModuleStatus {
    Running,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
#[allow(dead_code)] // Degraded/Unhealthy are produced by real modules' health checks.
pub enum ModuleHealth {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleLogEntry {
    pub at: String,
    pub level: LogLevel,
    pub message: String,
}

/// Bounded in-memory log per module.
#[derive(Debug, Default)]
pub struct LogBuffer {
    entries: VecDeque<ModuleLogEntry>,
}

impl LogBuffer {
    const CAPACITY: usize = 200;

    pub fn push(&mut self, level: LogLevel, message: impl Into<String>) {
        if self.entries.len() == Self::CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(ModuleLogEntry {
            at: chrono::Utc::now().to_rfc3339(),
            level,
            message: message.into(),
        });
    }

    pub fn entries(&self) -> Vec<ModuleLogEntry> {
        self.entries.iter().cloned().collect()
    }
}
