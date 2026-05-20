use serde::{Deserialize, Serialize};

/// AI 配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AIConfig {
    pub provider: String,
    #[serde(alias = "apiKey")]
    pub api_key: String,
    #[serde(alias = "apiUrl")]
    pub api_url: String,
    pub model: String,
    pub temperature: f32,
    #[serde(alias = "maxTokens")]
    pub max_tokens: i32,
}

/// 工具调用定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDefinition {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// 聊天消息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    /// 创建普通消息
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            role: role.to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
}

/// AI 响应（API 返回格式）
#[derive(Debug, Deserialize)]
pub struct AIApiResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub content: String,
}

/// 章节摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSummary {
    pub id: String,
    pub chapter_id: String,
    pub short_summary: String,
    pub long_summary: String,
    pub tags: Vec<String>,
    pub characters: Vec<String>,
    pub locations: Vec<String>,
    pub events: Vec<String>,
    pub generated_at: i64,
    pub is_confirmed: bool,
}

/// AI 摘要响应
#[derive(Debug, Deserialize)]
pub struct SummaryResponse {
    pub short_summary: String,
    pub long_summary: String,
    pub tags: Vec<String>,
    pub characters: Vec<String>,
    pub locations: Vec<String>,
    pub events: Vec<String>,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}



/// 流式响应事件
#[derive(Debug, Clone, Serialize)]
pub struct StreamEvent {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub chunk: String,
    #[serde(rename = "isComplete")]
    pub is_complete: bool,
    #[serde(rename = "isToolCall")]
    pub is_tool_call: bool,
    #[serde(rename = "toolName")]
    pub tool_name: Option<String>,
    #[serde(rename = "toolDisplayName")]
    pub tool_display_name: Option<String>,
    #[serde(rename = "toolParams")]
    pub tool_params: Option<serde_json::Value>,
}
