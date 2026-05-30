use serde::{Deserialize, Serialize};

/// 章节
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Chapter {
    pub id: String,
    pub title: String,
    #[sqlx(rename = "order")]
    pub order: i32,
    pub volume_id: String,
    pub content: String,
    pub word_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 卷
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Volume {
    pub id: String,
    pub title: String,
    #[sqlx(rename = "order")]
    pub order: i32,
    pub is_collapsed: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 书籍
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub volumes: Vec<Volume>,
    pub chapters: Vec<Chapter>,
    pub current_chapter_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 书籍列表项（简化版）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BookListItem {
    pub id: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub updated_at: i64,
}
