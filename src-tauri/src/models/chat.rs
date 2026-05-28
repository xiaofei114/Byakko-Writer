use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// AI 对话消息结构
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AIChatMessage {
    pub id: String,
    pub book_id: String,
    pub chapter_id: Option<String>,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub context_type: Option<String>,
    pub timestamp: i64,
    pub polish_handled: i32,
    pub handled_status: Option<String>,
}

/// 对话会话结构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    pub session_id: String,
    pub book_id: String,
    pub chapter_id: Option<String>,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: i32,
}


