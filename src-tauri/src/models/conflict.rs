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
    pub detected_at: i64,
    pub is_ignored: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignored_at: Option<i64>,
}
