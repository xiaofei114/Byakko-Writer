use crate::models::{ChapterSnapshot, SnapshotDiff};
use crate::services::snapshot_service;

/// 创建章节快照
#[tauri::command]
pub async fn create_chapter_snapshot(
    book_id: String,
    chapter_id: String,
    chapter_title: String,
    name: String,
    content: String,
) -> Result<ChapterSnapshot, String> {
    snapshot_service::create_chapter_snapshot(book_id, chapter_id, chapter_title, name, content)
        .await
        .map_err(|e| e.to_string())
}

/// 获取章节的快照列表
#[tauri::command]
pub async fn list_chapter_snapshots(
    book_id: String,
    chapter_id: String,
) -> Result<Vec<ChapterSnapshot>, String> {
    snapshot_service::list_chapter_snapshots(book_id, chapter_id).await.map_err(|e| e.to_string())
}

/// 获取单个快照详情
#[tauri::command]
pub async fn get_chapter_snapshot(
    book_id: String,
    snapshot_id: String,
) -> Result<ChapterSnapshot, String> {
    snapshot_service::get_chapter_snapshot(book_id, snapshot_id).await.map_err(|e| e.to_string())
}

/// 删除快照
#[tauri::command]
pub async fn delete_chapter_snapshot(
    book_id: String,
    snapshot_id: String,
) -> Result<(), String> {
    snapshot_service::delete_chapter_snapshot(book_id, snapshot_id).await.map_err(|e| e.to_string())
}

/// 清理章节快照（保留最近 N 个）
#[tauri::command]
pub async fn cleanup_chapter_snapshots(
    book_id: String,
    chapter_id: String,
    keep_count: i32,
) -> Result<i32, String> {
    snapshot_service::cleanup_chapter_snapshots(book_id, chapter_id, keep_count)
        .await
        .map_err(|e| e.to_string())
}

/// 对比两个快照
#[tauri::command]
pub async fn compare_snapshots(
    book_id: String,
    snapshot_id1: String,
    snapshot_id2: String,
) -> Result<SnapshotDiff, String> {
    snapshot_service::compare_snapshots(book_id, snapshot_id1, snapshot_id2)
        .await
        .map_err(|e| e.to_string())
}
