use serde::{Deserialize, Serialize};

/// AI 抽取出的实体草稿
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CanonEntityDraft {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub attrs: String,
}

/// 状态变化（如“境界=筑基三层”“位置=青阳城”）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CanonStateDraft {
    #[serde(default)]
    pub entity: String,
    #[serde(default)]
    pub field: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub quote: String,
}

/// 事件（append-only）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CanonEventDraft {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub predicate: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub importance: String,
}

/// 单章抽取结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CanonExtraction {
    #[serde(default)]
    pub entities: Vec<CanonEntityDraft>,
    #[serde(default)]
    pub states: Vec<CanonStateDraft>,
    #[serde(default)]
    pub events: Vec<CanonEventDraft>,
}
