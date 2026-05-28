use tauri::AppHandle;
use crate::models::{AIConfig, ChapterSummary, ChatSession, AIChatMessage};
use crate::services::{summary_service, chat_service, summary_generator_service};

/// 生成章节摘要
#[tauri::command]
pub async fn generate_chapter_summary(
    chapter_id: String,
    chapter_title: String,
    content: String,
    config: AIConfig,
) -> Result<ChapterSummary, String> {
    summary_service::generate_chapter_summary(chapter_id, chapter_title, content, config)
        .await
        .map_err(|e| e.to_string())
}

/// 加载章节摘要
#[tauri::command]
pub async fn load_chapter_summary(chapter_id: String) -> Result<Option<ChapterSummary>, String> {
    summary_service::load_chapter_summary(chapter_id).await.map_err(|e| e.to_string())
}

/// 发送流式聊天消息
#[tauri::command]
pub async fn send_chat_message_stream(
    app: AppHandle,
    session_id: Option<String>,
    book_id: String,
    chapter_id: Option<String>,
    message: String,
    config: AIConfig,
) -> Result<String, String> {
    chat_service::send_chat_message_stream(app, session_id, book_id, chapter_id, message, config)
        .await
        .map_err(|e| e.to_string())
}

/// 获取聊天历史
#[tauri::command]
pub async fn get_chat_history(
    session_id: String,
    limit: Option<i32>,
) -> Result<Vec<AIChatMessage>, String> {
    let limit = limit.unwrap_or(50) as i64;
    chat_service::get_chat_history(&session_id, limit).await.map_err(|e| e.to_string())
}

/// 获取会话列表
#[tauri::command]
pub async fn get_chat_sessions(book_id: String) -> Result<Vec<ChatSession>, String> {
    chat_service::get_chat_sessions(&book_id).await.map_err(|e| e.to_string())
}

/// 删除会话
#[tauri::command]
pub async fn delete_chat_session(session_id: String) -> Result<(), String> {
    chat_service::delete_chat_session(&session_id).await.map_err(|e| e.to_string())
}

/// 更新消息的 polish_handled 状态
#[tauri::command]
pub async fn update_message_polish_handled(message_id: String, handled: bool) -> Result<(), String> {
    chat_service::update_message_polish_handled(&message_id, handled).await.map_err(|e| e.to_string())
}

/// 更新单行修改消息处理状态
#[tauri::command]
pub async fn update_line_edit_handled_status(message_id: String, handled_status: String) -> Result<(), String> {
    chat_service::update_line_edit_handled_status(&message_id, &handled_status).await.map_err(|e| e.to_string())
}

/// 发送润色请求（独立流程）
#[tauri::command]
pub async fn send_polish_request(
    app: AppHandle,
    session_id: Option<String>,
    book_id: String,
    chapter_id: Option<String>,
    original_text: String,
    instruction: String,
    config: AIConfig,
) -> Result<String, String> {
    chat_service::send_polish_request(app, session_id, book_id, chapter_id, original_text, instruction, config)
        .await
        .map_err(|e| e.to_string())
}

/// 批量生成章节摘要
#[tauri::command]
pub async fn batch_generate_chapter_summaries(
    chapter_ids: Vec<String>,
    config: AIConfig,
    max_concurrent: Option<usize>,
) -> Result<summary_generator_service::SummaryGenerationResult, String> {
    let max_concurrent = max_concurrent.unwrap_or(3);
    summary_generator_service::batch_generate_summaries(chapter_ids, config, max_concurrent)
        .await
        .map_err(|e| e.to_string())
}

/// 生成所有缺失的章节摘要
#[tauri::command]
pub async fn generate_missing_summaries(
    book_id: String,
    config: AIConfig,
) -> Result<summary_generator_service::SummaryGenerationResult, String> {
    summary_generator_service::generate_missing_summaries_background(book_id, config)
        .await
        .map_err(|e| e.to_string())
}

/// 获取摘要生成状态
#[tauri::command]
pub async fn get_summary_generation_status() -> Result<summary_generator_service::GenerationStatus, String> {
    Ok(summary_generator_service::get_generation_status())
}
