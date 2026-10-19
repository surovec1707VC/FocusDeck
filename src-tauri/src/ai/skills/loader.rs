//! Reads `skills/*.json` manifests from disk.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyLevel {
    ReadOnly,
    ConfirmBeforeAction,
    Dangerous,
}

impl SafetyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::ConfirmBeforeAction => "confirm_before_action",
            Self::Dangerous => "dangerous",
        }
    }
}

/// Skill manifest. Unknown fields are rejected so typos surface as load errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    pub required_permissions: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub system_prompt: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub safety_level: SafetyLevel,
    pub max_steps: u32,
    pub model_profile: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadIssue {
    pub file: String,
    pub errors: Vec<String>,
}

pub struct LoadedManifest {
    pub path: PathBuf,
    pub manifest: SkillManifest,
}

/// Parses every `*.json` file in `dir` (sorted by name). Files that fail to
/// read or parse are returned as issues instead of aborting the whole load.
pub fn load_dir(dir: &Path) -> (Vec<LoadedManifest>, Vec<LoadIssue>) {
    let mut loaded = Vec::new();
    let mut issues = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            issues.push(LoadIssue {
                file: dir.display().to_string(),
                errors: vec![format!("cannot read skills directory: {e}")],
            });
            return (loaded, issues);
        }
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();

    for path in paths {
        let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<SkillManifest>(&text).map_err(|e| e.to_string()));
        match parsed {
            Ok(manifest) => loaded.push(LoadedManifest { path, manifest }),
            Err(e) => issues.push(LoadIssue { file, errors: vec![e] }),
        }
    }
    (loaded, issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_manifests_parse() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../skills");
        let (loaded, issues) = load_dir(&dir);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(loaded.len(), 14);
    }
}
