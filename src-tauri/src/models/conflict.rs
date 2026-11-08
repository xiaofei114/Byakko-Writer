use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 检测到的设定冲突
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DetectedConflict {
    pub id: String,
    pub book_id: String,
    pub description: String,
    pub suggestion: String,
    /// 严重程度：high / medium / low
    pub severity: String,
    pub detected_at: i64,
    pub is_ignored: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignored_at: Option<i64>,
    /// 冲突涉及的章节（可空）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter_id: Option<String>,
    /// 章节改稿后置 1，提示「可能已修复，待复核」
    pub needs_review: i32,
    /// 标记已解决的时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDetectionStatusEvent {
    pub book_id: String,
    pub source: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_count: Option<usize>,
    pub occurred_at: i64,
}
