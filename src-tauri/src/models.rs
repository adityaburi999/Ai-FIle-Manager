use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionMode {
    Ignore,
    ReadOnly,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExclusionRule {
    pub path: String,
    pub excluded: bool,
    pub mode: ExclusionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileClassification {
    pub category: String,
    pub sub_category: String,
    pub tags: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDecision {
    pub action: String,
    pub confidence: f32,
    pub reason: String,
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLog {
    pub timestamp: String,
    pub action: String,
    pub source: String,
    pub destination: String,
    pub reason: String,
    pub model_confidence: f32,
    pub rollback_group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSummary {
    pub scanned: usize,
    pub moved: usize,
    pub skipped: usize,
    pub duplicates: usize,
    pub indexed: usize,
    pub rollback_group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub relevance_score: f32,
    pub preview_metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderStatus {
    pub path: String,
    pub watching: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub watched_folders: Vec<FolderStatus>,
    pub index_root: String,
    pub database_path: String,
}
