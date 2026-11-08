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

/// 差异块类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffBlockType {
    Equal,
    Insert,
    Delete,
}

impl DiffBlockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiffBlockType::Equal => "equal",
            DiffBlockType::Insert => "insert",
            DiffBlockType::Delete => "delete",
        }
    }
}

/// 差异块（前端 SnapshotManager 按 block_type / old_text / new_text 渲染）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffBlock {
    pub block_type: String,
    pub old_text: String,
    pub new_text: String,
}

/// 快照对比结果（字段名与前端 types/index.ts 的 SnapshotComparison 保持一致）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotComparison {
    pub old_snapshot: ChapterSnapshot,
    pub new_snapshot: ChapterSnapshot,
    pub diff_blocks: Vec<DiffBlock>,
}
