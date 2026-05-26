use serde::{Deserialize, Serialize};

/// 分卷梗概
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VolumeSummaryItem {
    pub title: String,
    pub summary: String,
}

/// 事件时间线条目
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventTimelineItem {
    pub chapter: i32,
    pub title: String,
    pub event: String,
    pub impact: String,
    pub arc: String,
}

/// 关键角色状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyCharacterStatus {
    pub name: String,
    pub status: String,
    pub location: String,
}

/// 故事记忆（DB 行映射）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryMemory {
    pub book_id: String,
    pub book_summary: String,
    pub volume_summaries: Vec<VolumeSummaryItem>,
    pub event_timeline: Vec<EventTimelineItem>,
    pub protagonist_status: String,
    pub key_character_statuses: Vec<KeyCharacterStatus>,
    pub unresolved_threads: Vec<String>,
    pub world_rules: String,
    pub last_chapter_count: i64,
    pub last_word_count: i64,
    pub updated_at: i64,
}

/// AI 更新 Story Bible 的输入参数（供前端调用时使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct StoryMemoryUpdateParams {
    pub book_id: String,
    pub force: bool,
}

/// 分组摘要缓存（供未来直接查询分组摘要）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct StoryMemoryGroup {
    pub book_id: String,
    pub group_index: i32,
    pub start_chapter: i32,
    pub end_chapter: i32,
    pub chapter_ids: Vec<String>,
    pub summary: String,
    pub word_count: i64,
    pub generated_at: i64,
}

/// 分组处理进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupProgress {
    pub group_index: i32,
    pub start_chapter: i32,
    pub end_chapter: i32,
    pub status: String, // "cached" | "generated" | "error"
    pub message: String,
}

/// AI 更新返回结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryMemoryUpdateResult {
    pub success: bool,
    pub message: String,
    pub chapter_count: i32,
    pub total_word_count: i64,
    pub groups: Vec<GroupProgress>,
    pub groups_cached: i32,
    pub groups_generated: i32,
}
