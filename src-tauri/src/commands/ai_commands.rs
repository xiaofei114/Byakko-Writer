use tauri::AppHandle;
use crate::models::{AIConfig, ChapterSummary, ChatSession, AIChatMessage};
use crate::services::{ai_service, summary_service, chat_service};

/// 发送 AI 消息（非流式）
#[tauri::command]
pub async fn send_ai_message(
    message: String,
    history: Vec<crate::models::ChatMessage>,
    config: AIConfig,
) -> Result<String, String> {
    ai_service::send_ai_message(message, history, config).await.map_err(|e| e.to_string())
}

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

/// 确认章节摘要
#[tauri::command]
pub async fn confirm_chapter_summary(chapter_id: String) -> Result<(), String> {
    summary_service::confirm_chapter_summary(chapter_id).await.map_err(|e| e.to_string())
}

/// 批量生成摘要
#[tauri::command]
pub async fn batch_generate_summaries(
    chapter_ids: Vec<String>,
    config: AIConfig,
) -> Result<Vec<serde_json::Value>, String> {
    let results = summary_service::batch_generate_summaries(chapter_ids, config).await
        .map_err(|e| e.to_string())?;
    
    let json_results: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(id, result)| {
            match result {
                Ok(summary) => serde_json::json!({
                    "chapterId": id,
                    "success": true,
                    "summary": summary
                }),
                Err(e) => serde_json::json!({
                    "chapterId": id,
                    "success": false,
                    "error": e.to_string()
                }),
            }
        })
        .collect();
    
    Ok(json_results)
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

/// 润色文本
#[tauri::command]
pub async fn polish_text(
    text: String,
    config: AIConfig,
) -> Result<String, String> {
    use crate::services::ai_service;
    ai_service::polish_text(&text, config).await.map_err(|e| e.to_string())
}
