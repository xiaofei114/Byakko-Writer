use crate::models::{ChapterSnapshot, SnapshotComparison};
use crate::services::{book_service, snapshot_service};

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
    old_snapshot_id: String,
    new_snapshot_id: String,
) -> Result<SnapshotComparison, String> {
    snapshot_service::compare_snapshots(book_id, old_snapshot_id, new_snapshot_id)
        .await
        .map_err(|e| e.to_string())
}

/// 恢复快照的结果
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSnapshotResult {
    pub chapter_id: String,
    pub chapter_title: String,
    pub content: String,
    pub word_count: i64,
}

/// 恢复章节快照
///
/// 与旧实现（只改前端缓存）不同，这里会：
/// 1. 先把当前内容存一份「恢复前」快照
/// 2. 正文与标题真正落库（顺带触发摘要/账本重抽、冲突待复核）
#[tauri::command]
pub async fn restore_chapter_snapshot(
    book_id: String,
    snapshot_id: String,
) -> Result<RestoreSnapshotResult, String> {
    let snapshot = snapshot_service::get_chapter_snapshot(book_id.clone(), snapshot_id)
        .await
        .map_err(|e| e.to_string())?;

    let current_content = book_service::load_chapter_content(snapshot.chapter_id.clone())
        .await
        .unwrap_or_default();

    if !current_content.trim().is_empty() && current_content != snapshot.content {
        let current_title = chapter_title(&snapshot.chapter_id)
            .await
            .unwrap_or_else(|| snapshot.chapter_title.clone());
        let _ = snapshot_service::create_chapter_snapshot(
            book_id.clone(),
            snapshot.chapter_id.clone(),
            current_title,
            "恢复前".to_string(),
            current_content,
        )
        .await;
    }

    book_service::save_chapter_content(snapshot.chapter_id.clone(), snapshot.content.clone())
        .await
        .map_err(|e| e.to_string())?;

    // 回滚后旧摘要必然过期：删掉让自动摘要重新生成
    if let Err(e) =
        crate::services::summary_service::invalidate_chapter_summary(&snapshot.chapter_id).await
    {
        log::warn!("[Snapshot] 失效旧摘要失败: {}", e);
    }

    if !snapshot.chapter_title.trim().is_empty() {
        book_service::update_chapter_title(
            snapshot.chapter_id.clone(),
            snapshot.chapter_title.clone(),
        )
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(RestoreSnapshotResult {
        word_count: snapshot.content.chars().count() as i64,
        chapter_id: snapshot.chapter_id,
        chapter_title: snapshot.chapter_title,
        content: snapshot.content,
    })
}

async fn chapter_title(chapter_id: &str) -> Option<String> {
    let pool = crate::db::get_pool().await.ok()?;
    sqlx::query_scalar("SELECT title FROM chapters WHERE id = ?1")
        .bind(chapter_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}
