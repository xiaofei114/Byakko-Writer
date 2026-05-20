use crate::models::{LearnStyleParams, LearnStyleResult, UpdateStyleParams, StylePromptResult, UserWritingStyle, BookWritingStyle};
use crate::services::style_service;

/// 学习用户写作风格
#[tauri::command]
pub async fn learn_writing_style(params: LearnStyleParams) -> Result<LearnStyleResult, String> {
    style_service::learn_writing_style(params)
        .await
        .map_err(|e| e.to_string())
}

/// 获取风格提示词（用于 AI 调用）
#[tauri::command]
pub async fn get_style_prompt(book_id: Option<String>) -> Result<StylePromptResult, String> {
    style_service::get_style_prompt(book_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// 更新风格配置
#[tauri::command]
pub async fn update_style_config(params: UpdateStyleParams) -> Result<(), String> {
    style_service::update_style_config(params)
        .await
        .map_err(|e| e.to_string())
}

/// 获取用户全局写作风格
#[tauri::command]
pub async fn get_user_writing_style() -> Result<Option<UserWritingStyle>, String> {
    style_service::get_user_writing_style()
        .await
        .map_err(|e| e.to_string())
}

/// 获取书籍级写作风格
#[tauri::command]
pub async fn get_book_writing_style(book_id: String) -> Result<Option<BookWritingStyle>, String> {
    style_service::get_book_writing_style(&book_id)
        .await
        .map_err(|e| e.to_string())
}

/// 检查是否需要学习风格（章节数达到阈值时）
#[tauri::command]
pub async fn check_should_learn_style(book_id: String) -> Result<bool, String> {
    let pool = crate::db::get_pool().await.map_err(|e| e.to_string())?;
    
    // 检查是否已有风格配置
    let has_style = sqlx::query("SELECT 1 FROM book_writing_styles WHERE book_id = ?1 AND is_enabled = 1")
        .bind(&book_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .is_some();
    
    if has_style {
        return Ok(false); // 已有风格，不需要再学习
    }
    
    // 检查章节数是否达到阈值（3章）
    let chapter_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chapters WHERE book_id = ?1")
        .bind(&book_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(chapter_count >= 3)
}

/// 手动触发自动风格分析
#[tauri::command]
pub async fn trigger_style_analysis(book_id: Option<String>) -> Result<String, String> {
    crate::services::auto_style_service::trigger_manual_style_analysis(book_id)
        .await
        .map_err(|e| e.to_string())
}
