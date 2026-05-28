use crate::models::{AIConfig, ConflictDetectionStatus, DetectedConflict};
use crate::services::conflict_service;

/// 执行冲突检测（独立流程，不依赖聊天上下文）
#[tauri::command]
pub async fn run_conflict_detection(
    book_id: String,
    config: AIConfig,
) -> Result<Vec<DetectedConflict>, String> {
    conflict_service::run_detection(&book_id, &config)
        .await
        .map_err(|e| e.to_string())
}

/// 忽略指定冲突
#[tauri::command]
pub async fn ignore_conflict(conflict_id: String) -> Result<(), String> {
    conflict_service::ignore_conflict(&conflict_id)
        .await
        .map_err(|e| e.to_string())
}

/// 取消忽略指定冲突
#[tauri::command]
pub async fn unignore_conflict(conflict_id: String) -> Result<(), String> {
    conflict_service::unignore_conflict(&conflict_id)
        .await
        .map_err(|e| e.to_string())
}

/// 删除指定冲突
#[tauri::command]
pub async fn delete_conflict(conflict_id: String) -> Result<(), String> {
    conflict_service::delete_conflict(&conflict_id)
        .await
        .map_err(|e| e.to_string())
}

/// 获取活跃的（未忽略的）冲突列表
#[tauri::command]
pub async fn get_active_conflicts(book_id: String) -> Result<Vec<DetectedConflict>, String> {
    conflict_service::get_active_conflicts(&book_id)
        .await
        .map_err(|e| e.to_string())
}

/// 获取所有冲突（包括已忽略的）
#[tauri::command]
pub async fn get_all_conflicts(book_id: String) -> Result<Vec<DetectedConflict>, String> {
    conflict_service::get_all_conflicts(&book_id)
        .await
        .map_err(|e| e.to_string())
}

/// 获取最近一次冲突检测状态
#[tauri::command]
pub async fn get_conflict_detection_status(book_id: String) -> Result<Option<ConflictDetectionStatus>, String> {
    conflict_service::get_conflict_detection_status(&book_id)
        .await
        .map_err(|e| e.to_string())
}
