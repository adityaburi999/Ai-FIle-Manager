use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use anyhow::Context;
use chrono::Utc;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    classifier::FileIntelligenceEngine,
    db,
    indexer::IndexManager,
    models::{
        ActionLog, AiDecision, ExclusionMode, ExclusionRule, FolderStatus, OrganizationSummary,
        SearchResult, SystemStatus,
    },
};

struct WatchHandle {
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

pub struct FileManagerService {
    db_path: PathBuf,
    index_root: PathBuf,
    maps_root: PathBuf,
    classifier: FileIntelligenceEngine,
    indexer: IndexManager,
    watchers: HashMap<String, WatchHandle>,
}

impl FileManagerService {
    pub fn new(work_root: &Path) -> anyhow::Result<Self> {
        let engine_root = work_root.join(".ai_file_manager");
        let db_path = engine_root.join("metadata.sqlite");
        let index_root = engine_root.join("index");
        let maps_root = engine_root.join(".ai_maps");

        std::fs::create_dir_all(&engine_root)?;
        std::fs::create_dir_all(&maps_root)?;
        db::init(&db_path)?;

        let indexer = IndexManager::new(&index_root)?;

        Ok(Self {
            db_path,
            index_root,
            maps_root,
            classifier: FileIntelligenceEngine,
            indexer,
            watchers: HashMap::new(),
        })
    }

    pub fn status(&self) -> SystemStatus {
        SystemStatus {
            watched_folders: self
                .watchers
                .keys()
                .map(|p| FolderStatus {
                    path: p.clone(),
                    watching: true,
                })
                .collect(),
            index_root: self.index_root.display().to_string(),
            database_path: self.db_path.display().to_string(),
        }
    }

    pub fn set_exclusion_rule(&self, rule: ExclusionRule) -> anyhow::Result<()> {
        db::set_exclusion_rule(&self.db_path, &rule)
    }

    pub fn list_logs(&self, limit: usize) -> anyhow::Result<Vec<ActionLog>> {
        db::list_logs(&self.db_path, limit.max(1))
    }

    pub fn rollback(&self, rollback_group: String) -> anyhow::Result<usize> {
        db::rollback_moves(&self.db_path, &rollback_group)
    }

    pub fn semantic_search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
        self.indexer.search(query, limit)
    }

    pub fn organize_directory(&self, target_dir: &Path) -> anyhow::Result<OrganizationSummary> {
        if !target_dir.exists() {
            anyhow::bail!("target directory does not exist: {}", target_dir.display());
        }

        let rollback_group = Uuid::new_v4().to_string();
        let mut scanned = 0usize;
        let mut moved = 0usize;
        let mut skipped = 0usize;
        let mut duplicates = 0usize;
        let mut indexed = 0usize;

        for entry in WalkDir::new(target_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            scanned += 1;
            match self.organize_file(target_dir, entry.path(), &rollback_group, true)? {
                FileOp::Moved => {
                    moved += 1;
                    indexed += 1;
                }
                FileOp::IndexedOnly => {
                    indexed += 1;
                }
                FileOp::Duplicate => {
                    duplicates += 1;
                }
                FileOp::Skipped => {
                    skipped += 1;
                }
            }
        }

        self.generate_file_map(target_dir)?;

        Ok(OrganizationSummary {
            scanned,
            moved,
            skipped,
            duplicates,
            indexed,
            rollback_group,
        })
    }

    pub fn set_continuous_mode(this: Arc<RwLock<Self>>, target_dir: &Path, enabled: bool) -> anyhow::Result<()> {
        let key = target_dir.canonicalize()?.display().to_string();

        if enabled {
            let mut guard = this.blocking_write();
            if guard.watchers.contains_key(&key) {
                return Ok(());
            }

            let db_path = guard.db_path.clone();
            let maps_root = guard.maps_root.clone();
            let indexer = guard.indexer.clone();
            let classifier = guard.classifier.clone();
            let root = target_dir.to_path_buf();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_signal = Arc::clone(&stop);

            let join = thread::spawn(move || {
                let (tx, rx) = std::sync::mpsc::channel();
                let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
                    Ok(w) => w,
                    Err(_) => return,
                };

                if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
                    return;
                }

                while !stop_signal.load(Ordering::Relaxed) {
                    match rx.recv_timeout(Duration::from_millis(500)) {
                        Ok(Ok(event)) => {
                            if matches!(
                                event.kind,
                                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
                            ) {
                                for path in event.paths {
                                    if path.is_file() {
                                        let _ = process_file(
                                            &db_path,
                                            &indexer,
                                            &classifier,
                                            &maps_root,
                                            &root,
                                            &path,
                                            "watcher-event",
                                            false,
                                        );
                                    }
                                }
                            }
                        }
                        Ok(Err(_)) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });

            guard.watchers.insert(
                key,
                WatchHandle {
                    stop,
                    join: Some(join),
                },
            );
            return Ok(());
        }

        let mut guard = this.blocking_write();
        if let Some(mut handle) = guard.watchers.remove(&key) {
            handle.stop.store(true, Ordering::Relaxed);
            if let Some(join) = handle.join.take() {
                let _ = join.join();
            }
        }

        Ok(())
    }

    fn organize_file(
        &self,
        base_dir: &Path,
        file_path: &Path,
        rollback_group: &str,
        manual_trigger: bool,
    ) -> anyhow::Result<FileOp> {
        process_file(
            &self.db_path,
            &self.indexer,
            &self.classifier,
            &self.maps_root,
            base_dir,
            file_path,
            rollback_group,
            manual_trigger,
        )
    }

    fn generate_file_map(&self, folder: &Path) -> anyhow::Result<()> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let mut by_category: HashMap<String, Vec<String>> = HashMap::new();

        for entry in WalkDir::new(folder)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let p = entry.path().display().to_string();
            let class = self.classifier.classify(entry.path());
            nodes.push(serde_json::json!({
                "id": p,
                "category": class.category,
                "sub_category": class.sub_category,
            }));

            by_category
                .entry(class.category)
                .or_default()
                .push(entry.path().display().to_string());
        }

        for files in by_category.values() {
            for pair in files.windows(2) {
                edges.push(serde_json::json!({
                    "source": pair[0],
                    "target": pair[1],
                    "relationship": "category_similarity"
                }));
            }
        }

        let map = serde_json::json!({
            "folder": folder.display().to_string(),
            "generated_at": Utc::now().to_rfc3339(),
            "nodes": nodes,
            "edges": edges,
        });

        let folder_id = folder
            .canonicalize()
            .unwrap_or_else(|_| folder.to_path_buf())
            .display()
            .to_string()
            .replace(['/', '\\', ':'], "_");
        let output = self.maps_root.join(format!("{folder_id}.json"));
        std::fs::write(output, serde_json::to_vec_pretty(&map)?)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum FileOp {
    Moved,
    IndexedOnly,
    Duplicate,
    Skipped,
}

fn process_file(
    db_path: &Path,
    indexer: &IndexManager,
    classifier: &FileIntelligenceEngine,
    _maps_root: &Path,
    base_dir: &Path,
    file_path: &Path,
    rollback_group: &str,
    manual_trigger: bool,
) -> anyhow::Result<FileOp> {
    let file_path = file_path
        .canonicalize()
        .with_context(|| format!("failed canonicalizing {}", file_path.display()))?;
    let file_path_str = file_path.display().to_string();

    if file_path_str.contains("/.ai_file_manager/") {
        return Ok(FileOp::Skipped);
    }

    if let Some(mode) = db::get_matching_exclusion_mode(db_path, &file_path_str)? {
        match mode {
            ExclusionMode::Ignore => return Ok(FileOp::Skipped),
            ExclusionMode::ReadOnly => {
                index_existing_file(db_path, indexer, classifier, &file_path)?;
                return Ok(FileOp::IndexedOnly);
            }
            ExclusionMode::Manual if !manual_trigger => return Ok(FileOp::Skipped),
            ExclusionMode::Manual => {}
        }
    }

    let class = classifier.classify(&file_path);
    let category_dir = base_dir.join(&class.category).join(&class.sub_category);
    std::fs::create_dir_all(&category_dir)?;

    let original_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file.bin");
    let mut target = category_dir.join(original_name);

    let current_hash = file_sha256(&file_path)?;
    if file_path != target {
        target = ensure_unique_destination(&target, &current_hash)?;
    }

    let decision = AiDecision {
        action: if file_path == target {
            "tag".to_string()
        } else {
            "move".to_string()
        },
        confidence: class.confidence,
        reason: "AI classification".to_string(),
        target_path: target.display().to_string(),
    };

    if file_path != target {
        if std::fs::rename(&file_path, &target).is_err() {
            std::fs::copy(&file_path, &target)?;
            std::fs::remove_file(&file_path)?;
        }

        db::insert_move_history(
            db_path,
            rollback_group,
            &file_path.display().to_string(),
            &target.display().to_string(),
        )?;

        db::insert_action_log(
            db_path,
            &ActionLog {
                timestamp: Utc::now().to_rfc3339(),
                action: decision.action,
                source: file_path.display().to_string(),
                destination: target.display().to_string(),
                reason: decision.reason,
                model_confidence: decision.confidence,
                rollback_group: rollback_group.to_string(),
            },
        )?;

        index_existing_file(db_path, indexer, classifier, &target)?;
        return Ok(FileOp::Moved);
    }

    index_existing_file(db_path, indexer, classifier, &file_path)?;
    Ok(FileOp::IndexedOnly)
}

fn index_existing_file(
    db_path: &Path,
    indexer: &IndexManager,
    classifier: &FileIntelligenceEngine,
    file_path: &Path,
) -> anyhow::Result<()> {
    let classification = classifier.classify(file_path);
    let content_preview = extract_preview(file_path);
    let hash = file_sha256(file_path)?;
    let path_str = file_path.display().to_string();
    let tags_json = serde_json::to_string(&classification.tags)?;
    let timestamp = Utc::now().to_rfc3339();

    db::upsert_file_record(
        db_path,
        &path_str,
        &classification.category,
        &classification.sub_category,
        &tags_json,
        &content_preview,
        &hash,
        &timestamp,
    )?;

    indexer.upsert_document(
        &path_str,
        &classification.category,
        &classification.sub_category,
        &classification.tags,
        &format!("{} {}", file_path.file_name().and_then(|n| n.to_str()).unwrap_or_default(), content_preview),
    )?;

    Ok(())
}

fn ensure_unique_destination(target: &Path, src_hash: &str) -> anyhow::Result<PathBuf> {
    if !target.exists() {
        return Ok(target.to_path_buf());
    }

    let existing_hash = file_sha256(target)?;
    if existing_hash == src_hash {
        return Ok(target.to_path_buf());
    }

    let stem = target
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = target.extension().and_then(|e| e.to_str()).unwrap_or_default();

    for idx in 1..10_000 {
        let name = if ext.is_empty() {
            format!("{stem}_{idx}")
        } else {
            format!("{stem}_{idx}.{ext}")
        };
        let candidate = target.with_file_name(name);
        if !candidate.exists() {
            return Ok(candidate);
        }

        let candidate_hash = file_sha256(&candidate)?;
        if candidate_hash == src_hash {
            return Ok(candidate);
        }
    }

    anyhow::bail!(
        "unable to find unique destination for {}",
        target.display()
    )
}

fn extract_preview(file_path: &Path) -> String {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();

    let textual = matches!(
        ext.as_str(),
        "txt" | "md" | "json" | "csv" | "rs" | "ts" | "tsx" | "js" | "jsx" | "py"
    );

    if !textual {
        return file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
    }

    let bytes = std::fs::read(file_path).unwrap_or_default();
    String::from_utf8_lossy(&bytes)
        .chars()
        .take(4096)
        .collect::<String>()
}

fn file_sha256(path: &Path) -> anyhow::Result<String> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(data);
    Ok(format!("{:x}", hasher.finalize()))
}
