use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 角色卡数据结构
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct CharacterCard {
    pub id: String,
    pub book_id: String,
    pub name: String,
    pub aliases: String, // JSON 数组
    pub gender: String,
    pub age: String,
    pub appearance: String,
    pub personality: String,
    pub background: String,
    pub goals: String,
    pub relationships: String, // JSON 数组
    pub tags: String, // JSON 数组
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 角色关系
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CharacterRelationship {
    pub target_character_id: String,
    pub target_name: String,
    pub relationship: String,
    pub description: String,
}

/// 角色卡创建/更新参数
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CharacterCardParams {
    pub name: String,
    pub aliases: Vec<String>,
    pub gender: String,
    pub age: String,
    pub appearance: String,
    pub personality: String,
    pub background: String,
    pub goals: String,
    pub relationships: Vec<CharacterRelationship>,
    pub tags: Vec<String>,
    pub notes: String,
}
