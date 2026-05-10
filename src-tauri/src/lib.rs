mod classifier;
mod db;
mod engine;
mod indexer;
mod models;

use std::{path::PathBuf, sync::Arc};

use engine::FileManagerService;
use models::{ActionLog, ExclusionRule, OrganizationSummary, SearchResult, SystemStatus};
use tauri::State;
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    service: Arc<RwLock<FileManagerService>>,
}

#[tauri::command]
async fn organize_directory(state: State<'_, AppState>, path: String) -> Result<OrganizationSummary, String> {
    let service = state.service.read().await;
    service
        .organize_directory(PathBuf::from(path).as_path())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_exclusion(state: State<'_, AppState>, rule: ExclusionRule) -> Result<(), String> {
    let service = state.service.read().await;
    service.set_exclusion_rule(rule).map_err(|e| e.to_string())
}

#[tauri::command]
async fn semantic_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    let service = state.service.read().await;
    service
        .semantic_search(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_logs(state: State<'_, AppState>, limit: Option<usize>) -> Result<Vec<ActionLog>, String> {
    let service = state.service.read().await;
    service.list_logs(limit.unwrap_or(50)).map_err(|e| e.to_string())
}

#[tauri::command]
async fn rollback_group(state: State<'_, AppState>, rollback_group: String) -> Result<usize, String> {
    let service = state.service.read().await;
    service.rollback(rollback_group).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_continuous_mode(
    state: State<'_, AppState>,
    path: String,
    enabled: bool,
) -> Result<(), String> {
    let service = Arc::clone(&state.service);
    engine::FileManagerService::set_continuous_mode(service, PathBuf::from(path).as_path(), enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn system_status(state: State<'_, AppState>) -> Result<SystemStatus, String> {
    let service = state.service.read().await;
    Ok(service.status())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cwd = std::env::current_dir().expect("cannot resolve current directory");
    let service = FileManagerService::new(&cwd).expect("failed to initialize file manager service");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            service: Arc::new(RwLock::new(service)),
        })
        .invoke_handler(tauri::generate_handler![
            organize_directory,
            set_exclusion,
            semantic_search,
            get_logs,
            rollback_group,
            set_continuous_mode,
            system_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
