use crate::models::{StoryMemory, StoryMemoryUpdateResult};
use crate::services::story_memory_service;

/// 获取故事记忆（完整数据结构）
#[tauri::command]
pub async fn get_story_memory(book_id: String) -> Result<Option<StoryMemory>, String> {
    story_memory_service::get_story_memory(&book_id)
        .await
        .map_err(|e| e.to_string())
}

/// 手动更新故事记忆
#[tauri::command]
pub async fn update_story_memory(
    app: tauri::AppHandle,
    book_id: String,
) -> Result<StoryMemoryUpdateResult, String> {
    story_memory_service::update_story_memory(&app, &book_id, false)
        .await
        .map_err(|e| e.to_string())
}

/// 强制重新生成故事记忆（清除所有缓存）
#[tauri::command]
pub async fn force_regenerate_story_memory(
    app: tauri::AppHandle,
    book_id: String,
) -> Result<StoryMemoryUpdateResult, String> {
    story_memory_service::update_story_memory(&app, &book_id, true)
        .await
        .map_err(|e| e.to_string())
}

/// 获取故事记忆的格式化文本
#[tauri::command]
pub async fn get_story_memory_text(book_id: String) -> Result<String, String> {
    story_memory_service::build_story_memory_text(&book_id)
        .await
        .map_err(|e| e.to_string())
}
