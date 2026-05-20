use serde::{Deserialize, Serialize};

/// 章节快照信息
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ChapterSnapshot {
    pub id: String,
    pub book_id: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub name: String,
    pub content: String,
    pub word_count: i64,
    pub created_at: String,
}

/// 快照对比结果
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDiff {
    pub added: Vec<DiffChunk>,
    pub removed: Vec<DiffChunk>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffChunk {
    pub text: String,
    pub start: usize,
    pub end: usize,
}
