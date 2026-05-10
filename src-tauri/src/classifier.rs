use std::path::Path;

use crate::models::FileClassification;

#[derive(Debug, Clone, Default)]
pub struct FileIntelligenceEngine;

impl FileIntelligenceEngine {
    pub fn classify(&self, file_path: &Path) -> FileClassification {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();

        let (category, sub_category, mut tags, confidence): (&str, &str, Vec<&str>, f32) =
            match extension.as_str() {
                "pdf" | "doc" | "docx" | "txt" | "md" => {
                    ("Documents", "Text", vec!["document", "text"], 0.88)
                }
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => {
                    ("Media", "Images", vec!["image", "media"], 0.9)
                }
                "mp4" | "mkv" | "mov" | "avi" => ("Media", "Video", vec!["video", "media"], 0.9),
                "mp3" | "wav" | "flac" => ("Media", "Audio", vec!["audio", "media"], 0.9),
                "zip" | "tar" | "gz" | "7z" | "rar" => {
                    ("Archives", "Compressed", vec!["archive", "compressed"], 0.87)
                }
                "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "c" | "cpp" | "h" => {
                    ("Code", "Source", vec!["code", "source"], 0.92)
                }
                "csv" | "xlsx" | "json" | "sql" => {
                    ("Data", "Structured", vec!["data", "structured"], 0.86)
                }
                _ => ("Other", "General", vec!["uncategorized"], 0.62),
            };

        if file_name.contains("invoice") || file_name.contains("receipt") {
            tags.push("finance");
        }
        if file_name.contains("resume") || file_name.contains("cv") {
            tags.push("career");
        }

        FileClassification {
            category: category.to_string(),
            sub_category: sub_category.to_string(),
            tags: tags.into_iter().map(ToString::to_string).collect(),
            confidence,
        }
    }
}
