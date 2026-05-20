use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 大纲数据结构
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct Outline {
    pub id: String,
    pub book_id: String,
    pub volume_id: Option<String>,
    pub chapter_id: Option<String>,
    pub outline_type: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 大纲统计信息
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OutlineStats {
    pub book_id: String,
    pub volume_id: Option<String>,
    pub chapter_id: Option<String>,
    pub has_coarse_outline: bool,
    pub has_fine_outline: bool,
    pub coarse_outline_updated_at: Option<i64>,
    pub fine_outline_updated_at: Option<i64>,
}
