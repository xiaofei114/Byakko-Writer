use crate::models::{Outline, OutlineStats};
use crate::services::outline_service;

/// 创建或更新大纲
#[tauri::command]
pub async fn save_outline(
    book_id: String,
    volume_id: Option<String>,
    chapter_id: Option<String>,
    outline_type: String,
    content: String,
) -> Result<Outline, String> {
    outline_service::save_outline(book_id, volume_id, chapter_id, outline_type, content)
        .await
        .map_err(|e| e.to_string())
}

/// 获取单个大纲
#[tauri::command]
pub async fn get_outline(outline_id: String) -> Result<Outline, String> {
    outline_service::get_outline(outline_id).await.map_err(|e| e.to_string())
}

/// 获取特定层级的大纲
#[tauri::command]
pub async fn get_outline_by_level(
    book_id: String,
    volume_id: Option<String>,
    chapter_id: Option<String>,
    outline_type: String,
) -> Result<Option<Outline>, String> {
    outline_service::get_outline_by_level(book_id, volume_id, chapter_id, outline_type)
        .await
        .map_err(|e| e.to_string())
}

/// 获取书籍的所有大纲
#[tauri::command]
pub async fn list_book_outlines(book_id: String) -> Result<Vec<Outline>, String> {
    outline_service::list_book_outlines(book_id).await.map_err(|e| e.to_string())
}

/// 获取卷的所有大纲
#[tauri::command]
pub async fn list_volume_outlines(
    book_id: String,
    volume_id: String,
) -> Result<Vec<Outline>, String> {
    outline_service::list_volume_outlines(book_id, volume_id).await.map_err(|e| e.to_string())
}

/// 获取章节的所有大纲
#[tauri::command]
pub async fn list_chapter_outlines(
    book_id: String,
    chapter_id: String,
) -> Result<Vec<Outline>, String> {
    outline_service::list_chapter_outlines(book_id, chapter_id).await.map_err(|e| e.to_string())
}

/// 删除大纲
#[tauri::command]
pub async fn delete_outline(outline_id: String) -> Result<(), String> {
    outline_service::delete_outline(outline_id).await.map_err(|e| e.to_string())
}

/// 获取大纲统计信息
#[tauri::command]
pub async fn get_outline_stats(
    book_id: String,
    volume_id: Option<String>,
    chapter_id: Option<String>,
) -> Result<OutlineStats, String> {
    outline_service::get_outline_stats(book_id, volume_id, chapter_id)
        .await
        .map_err(|e| e.to_string())
}
