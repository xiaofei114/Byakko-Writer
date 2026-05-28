use serde::{Deserialize, Serialize};

/// AI 配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AIConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(alias = "apiKey", default)]
    pub api_key: String,
    #[serde(alias = "apiUrl", default = "default_api_url")]
    pub api_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(alias = "maxTokens", default = "default_max_tokens")]
    pub max_tokens: i32,
    #[serde(alias = "maxRounds", default = "default_max_rounds")]
    pub max_rounds: i32,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            api_key: String::default(),
            api_url: default_api_url(),
            model: default_model(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            max_rounds: default_max_rounds(),
        }
    }
}

fn default_provider() -> String { "deepseek".to_string() }
fn default_api_url() -> String { "https://api.deepseek.com/v1".to_string() }
fn default_model() -> String { "deepseek-chat".to_string() }
fn default_temperature() -> f32 { 0.7 }
fn default_max_tokens() -> i32 { 10000 }
fn default_max_rounds() -> i32 { 30 }

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
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCallDefinition>>,
}

/// AI 决策结果（function calling 返回的结构化结果）
#[derive(Debug, Clone)]
pub struct AiDecision {
    /// 自然语言内容（直接回答时）
    pub content: Option<String>,
    /// 工具调用列表（需要执行工具时）
    pub tool_calls: Option<Vec<ToolCallDefinition>>,
}

impl AiDecision {
    /// 是否为工具调用
    pub fn is_tool_call(&self) -> bool {
        self.tool_calls.as_ref().map_or(false, |t| !t.is_empty())
    }
}

/// 工具定义（发送给 AI 的 function schema）
#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub def_type: String,
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plot_progression: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub emotional_beats: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub foreshadowing: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub unresolved_threads: Vec<String>,
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
    #[serde(default)]
    pub plot_progression: Option<String>,
    #[serde(default)]
    pub emotional_beats: Vec<String>,
    #[serde(default)]
    pub foreshadowing: Vec<String>,
    #[serde(default)]
    pub unresolved_threads: Vec<String>,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
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
