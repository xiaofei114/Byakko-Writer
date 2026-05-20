use serde::{Deserialize, Serialize};
use crate::models::AIConfig;

/// 应用配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub theme: String,
    pub primary_color: String,
    pub font_family: String,
    pub font_size: i32,
    pub line_height: f32,
    pub auto_save: bool,
    pub auto_save_interval: i32,
    pub ai: AIConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            primary_color: "#3498db".to_string(),
            font_family: "system".to_string(),
            font_size: 16,
            line_height: 1.8,
            auto_save: true,
            auto_save_interval: 30,
            ai: AIConfig {
                provider: "deepseek".to_string(),
                api_key: "".to_string(),
                api_url: "https://api.deepseek.com/v1".to_string(),
                model: "deepseek-chat".to_string(),
                temperature: 0.7,
                max_tokens: 10000,
            },
        }
    }
}
