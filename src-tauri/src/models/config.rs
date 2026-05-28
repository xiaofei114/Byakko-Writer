use serde::{Deserialize, Serialize};
use crate::models::AIConfig;

/// 应用配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_primary_color")]
    pub primary_color: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: i32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "default_auto_save")]
    pub auto_save: bool,
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval: i32,
    #[serde(default = "default_auto_story_memory")]
    pub auto_story_memory: bool,
    #[serde(default)]
    pub ai: AIConfig,
}

fn default_theme() -> String { "light".to_string() }
fn default_primary_color() -> String { "#3498db".to_string() }
fn default_font_family() -> String { "system".to_string() }
fn default_font_size() -> i32 { 16 }
fn default_line_height() -> f32 { 1.8 }
fn default_auto_save() -> bool { true }
fn default_auto_save_interval() -> i32 { 30 }
fn default_auto_story_memory() -> bool { true }

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
            auto_story_memory: true,
            ai: AIConfig {
                provider: "deepseek".to_string(),
                api_key: "".to_string(),
                api_url: "https://api.deepseek.com/v1".to_string(),
                model: "deepseek-chat".to_string(),
                temperature: 0.7,
                max_tokens: 10000,
                max_rounds: 30,
            },
        }
    }
}
